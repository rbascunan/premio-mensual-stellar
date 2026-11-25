#![no_std]

//! Premio mensual UpTeam
//!
//! Cada mes, el embajador con más puntos de la comunidad gana un premio fijo
//! (por ejemplo 50 USDC). La administradora carga los puntos y aprueba, y el
//! contrato decide al ganador y solo paga a partir del día 10 del mes siguiente.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    YaIniciado = 1,
    NoIniciado = 2,
    PeriodoInvalido = 3,
    PuntosInvalidos = 4,
    EmbajadorYaCargado = 5,
    PeriodoYaAprobado = 6,
    SinGanador = 7,
    NoAprobado = 8,
    YaPagado = 9,
    AntesDelDia10 = 10,
    MontoInvalido = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Periodo {
    pub ganador: Option<Address>,
    pub puntos_ganador: i128,
    pub aprobado: bool,
    pub pagado: bool,
    pub pagable_desde: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Token,
    Monto,
    Periodo(u32),
    Puntos(u32, Address),
}

#[contract]
pub struct PremioMensual;

#[contractimpl]
impl PremioMensual {
    /// Configura el contrato una sola vez: quién administra, en qué token se paga y cuánto.
    pub fn iniciar(env: Env, admin: Address, token: Address, monto: i128) -> Result<(), Error> {
        let storage = env.storage().instance();
        if storage.has(&DataKey::Admin) {
            return Err(Error::YaIniciado);
        }
        if monto <= 0 {
            return Err(Error::MontoInvalido);
        }
        storage.set(&DataKey::Admin, &admin);
        storage.set(&DataKey::Token, &token);
        storage.set(&DataKey::Monto, &monto);
        Ok(())
    }

    /// La administradora carga los puntos de un embajador para un mes (formato AAAAMM, ej. 202610).
    /// El contrato recuerda al de más puntos; en empate gana el primero en llegar a ese puntaje.
    pub fn cargar_puntos(env: Env, periodo: u32, embajador: Address, puntos: i128) -> Result<(), Error> {
        admin(&env)?.require_auth();
        if puntos <= 0 {
            return Err(Error::PuntosInvalidos);
        }
        let mut datos = leer_o_crear_periodo(&env, periodo)?;
        if datos.aprobado {
            return Err(Error::PeriodoYaAprobado);
        }

        let clave = DataKey::Puntos(periodo, embajador.clone());
        let persistente = env.storage().persistent();
        if persistente.has(&clave) {
            return Err(Error::EmbajadorYaCargado);
        }
        persistente.set(&clave, &puntos);

        if puntos > datos.puntos_ganador {
            datos.ganador = Some(embajador);
            datos.puntos_ganador = puntos;
        }
        persistente.set(&DataKey::Periodo(periodo), &datos);
        Ok(())
    }

    /// La administradora aprueba el premio del mes. Después ya no se pueden cargar puntos.
    pub fn aprobar(env: Env, periodo: u32) -> Result<(), Error> {
        admin(&env)?.require_auth();
        let mut datos = leer_periodo(&env, periodo)?;
        if datos.aprobado {
            return Err(Error::PeriodoYaAprobado);
        }
        if datos.ganador.is_none() {
            return Err(Error::SinGanador);
        }
        datos.aprobado = true;
        env.storage().persistent().set(&DataKey::Periodo(periodo), &datos);
        Ok(())
    }

    /// Paga el premio al ganador. Cualquiera puede llamarla: las reglas las controla el contrato.
    pub fn pagar(env: Env, periodo: u32) -> Result<Address, Error> {
        let mut datos = leer_periodo(&env, periodo)?;
        if !datos.aprobado {
            return Err(Error::NoAprobado);
        }
        if datos.pagado {
            return Err(Error::YaPagado);
        }
        if env.ledger().timestamp() < datos.pagable_desde {
            return Err(Error::AntesDelDia10);
        }
        let ganador = datos.ganador.clone().ok_or(Error::SinGanador)?;

        datos.pagado = true;
        env.storage().persistent().set(&DataKey::Periodo(periodo), &datos);

        let storage = env.storage().instance();
        let token_id: Address = storage.get(&DataKey::Token).ok_or(Error::NoIniciado)?;
        let monto: i128 = storage.get(&DataKey::Monto).ok_or(Error::NoIniciado)?;
        token::TokenClient::new(&env, &token_id).transfer(&env.current_contract_address(), &ganador, &monto);
        Ok(ganador)
    }

    /// Consulta el estado de un mes: ganador, puntos, si está aprobado, pagado y desde cuándo se puede pagar.
    pub fn consultar(env: Env, periodo: u32) -> Option<Periodo> {
        env.storage().persistent().get(&DataKey::Periodo(periodo))
    }
}

fn admin(env: &Env) -> Result<Address, Error> {
    env.storage().instance().get(&DataKey::Admin).ok_or(Error::NoIniciado)
}

fn leer_periodo(env: &Env, periodo: u32) -> Result<Periodo, Error> {
    env.storage().persistent().get(&DataKey::Periodo(periodo)).ok_or(Error::SinGanador)
}

fn leer_o_crear_periodo(env: &Env, periodo: u32) -> Result<Periodo, Error> {
    if let Some(datos) = env.storage().persistent().get(&DataKey::Periodo(periodo)) {
        return Ok(datos);
    }
    Ok(Periodo {
        ganador: None,
        puntos_ganador: 0,
        aprobado: false,
        pagado: false,
        pagable_desde: dia_10_del_mes_siguiente(periodo)?,
    })
}

/// Convierte un periodo AAAAMM en el timestamp UTC del día 10 del mes siguiente a las 00:00.
pub fn dia_10_del_mes_siguiente(periodo: u32) -> Result<u64, Error> {
    let anio = periodo / 100;
    let mes = periodo % 100;
    if !(2000..=2999).contains(&anio) || !(1..=12).contains(&mes) {
        return Err(Error::PeriodoInvalido);
    }
    let (anio_pago, mes_pago) = if mes == 12 { (anio + 1, 1) } else { (anio, mes + 1) };
    Ok(dias_desde_1970(anio_pago as i64, mes_pago as i64, 10) as u64 * 86_400)
}

// Algoritmo "days_from_civil" de Howard Hinnant (calendario gregoriano).
fn dias_desde_1970(anio: i64, mes: i64, dia: i64) -> i64 {
    let y = if mes <= 2 { anio - 1 } else { anio };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if mes > 2 { mes - 3 } else { mes + 9 };
    let doy = (153 * mp + 2) / 5 + dia - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod test;
