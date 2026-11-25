#![cfg(test)]

use super::{dia_10_del_mes_siguiente, Error, PremioMensual, PremioMensualClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

const PERIODO: u32 = 202610; // octubre 2026
const PREMIO: i128 = 50_0000000; // 50 unidades con 7 decimales, como USDC en Stellar

// 10 de noviembre de 2026, 00:00 UTC
const DIA_10_NOV_2026: u64 = 1_794_268_800;

struct Escenario<'a> {
    env: Env,
    premio: PremioMensualClient<'a>,
    token: TokenClient<'a>,
}

fn preparar() -> Escenario<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let emisor = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(emisor);
    let token_id = usdc.address();

    let contrato_id = env.register(PremioMensual, ());
    let premio = PremioMensualClient::new(&env, &contrato_id);
    premio.iniciar(&admin, &token_id, &PREMIO);

    // La comunidad fondea el contrato con 3 meses de premios.
    StellarAssetClient::new(&env, &token_id).mint(&contrato_id, &(PREMIO * 3));

    let token = TokenClient::new(&env, &token_id);
    Escenario { env, premio, token }
}

#[test]
fn calcula_el_dia_10_del_mes_siguiente() {
    assert_eq!(dia_10_del_mes_siguiente(PERIODO), Ok(DIA_10_NOV_2026));
    // Diciembre pasa al 10 de enero del año siguiente.
    assert_eq!(dia_10_del_mes_siguiente(202612), Ok(1_799_539_200));
    assert_eq!(dia_10_del_mes_siguiente(202613), Err(Error::PeriodoInvalido));
}

#[test]
fn gana_el_de_mas_puntos_y_cobra_el_premio() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    let luis = Address::generate(&e.env);
    let sofia = Address::generate(&e.env);

    e.premio.cargar_puntos(&PERIODO, &ana, &900);
    e.premio.cargar_puntos(&PERIODO, &luis, &1_240);
    e.premio.cargar_puntos(&PERIODO, &sofia, &300);
    e.premio.aprobar(&PERIODO);

    e.env.ledger().set_timestamp(DIA_10_NOV_2026);
    let ganador = e.premio.pagar(&PERIODO);

    assert_eq!(ganador, luis);
    assert_eq!(e.token.balance(&luis), PREMIO);
    assert_eq!(e.token.balance(&ana), 0);

    let estado = e.premio.consultar(&PERIODO).unwrap();
    assert!(estado.pagado);
    assert_eq!(estado.puntos_ganador, 1_240);
}

#[test]
fn no_paga_sin_aprobacion_de_la_administradora() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    e.premio.cargar_puntos(&PERIODO, &ana, &500);

    e.env.ledger().set_timestamp(DIA_10_NOV_2026);
    assert_eq!(e.premio.try_pagar(&PERIODO), Err(Ok(Error::NoAprobado)));
    assert_eq!(e.token.balance(&ana), 0);
}

#[test]
fn no_paga_antes_del_dia_10() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    e.premio.cargar_puntos(&PERIODO, &ana, &500);
    e.premio.aprobar(&PERIODO);

    e.env.ledger().set_timestamp(DIA_10_NOV_2026 - 1); // 9 de noviembre, 23:59:59
    assert_eq!(e.premio.try_pagar(&PERIODO), Err(Ok(Error::AntesDelDia10)));

    e.env.ledger().set_timestamp(DIA_10_NOV_2026);
    assert_eq!(e.premio.pagar(&PERIODO), ana);
}

#[test]
fn no_paga_dos_veces_el_mismo_mes() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    e.premio.cargar_puntos(&PERIODO, &ana, &500);
    e.premio.aprobar(&PERIODO);
    e.env.ledger().set_timestamp(DIA_10_NOV_2026);

    e.premio.pagar(&PERIODO);
    assert_eq!(e.premio.try_pagar(&PERIODO), Err(Ok(Error::YaPagado)));
    assert_eq!(e.token.balance(&ana), PREMIO);
}

#[test]
fn en_empate_gana_el_primero_en_llegar() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    let luis = Address::generate(&e.env);

    e.premio.cargar_puntos(&PERIODO, &ana, &800);
    e.premio.cargar_puntos(&PERIODO, &luis, &800);

    assert_eq!(e.premio.consultar(&PERIODO).unwrap().ganador, Some(ana));
}

#[test]
fn no_se_puede_cargar_dos_veces_ni_despues_de_aprobar() {
    let e = preparar();
    let ana = Address::generate(&e.env);
    let luis = Address::generate(&e.env);

    e.premio.cargar_puntos(&PERIODO, &ana, &500);
    assert_eq!(
        e.premio.try_cargar_puntos(&PERIODO, &ana, &5_000),
        Err(Ok(Error::EmbajadorYaCargado))
    );

    e.premio.aprobar(&PERIODO);
    assert_eq!(
        e.premio.try_cargar_puntos(&PERIODO, &luis, &5_000),
        Err(Ok(Error::PeriodoYaAprobado))
    );
}

#[test]
fn solo_la_administradora_puede_cargar_puntos() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env)).address();
    let contrato_id = env.register(PremioMensual, ());
    let premio = PremioMensualClient::new(&env, &contrato_id);
    premio.iniciar(&admin, &token_id, &PREMIO);

    // Sin la firma de la administradora, la llamada se rechaza.
    let intruso = Address::generate(&env);
    assert!(premio.try_cargar_puntos(&PERIODO, &intruso, &10_000).is_err());
}
