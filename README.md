# Premio mensual UpTeam - Contrato inteligente en Stellar

Autor: **Roberto Fernando Bascuñan Garello**

## El problema

En la comunidad Stellar Hispana, cada mes se paga un premio obligatorio de **50 USD** al embajador
que más puntos acumuló el mes anterior. Hoy ese pago mensual es manual:

1. La administradora espera a que estén cargados todos los puntos del mes.
2. Revisa el ranking a mano y decide quién ganó.
3. Busca la wallet del ganador, arma la transferencia y la firma.
4. Avisa a la comunidad quién ganó.

Cada paso puede fallar: un error al contar, un empate sin regla clara, un pago que se atrasa o se
olvida, o un pago doble. Y los embajadores no tienen cómo comprobar que el ganador fue el correcto
("¿seguro que yo no tenía más puntos?").

**La solución:** un contrato inteligente en Stellar que simplifica el pago mensual a **un paso
humano: aprobar.** El contrato guarda los puntos del mes, decide solo quién ganó, espera la
aprobación de la administradora y paga los 50 USDC a partir del día 10 del mes siguiente,
una sola vez y a la vista de todos.

| Antes (manual) | Con el contrato |
|---|---|
| La administradora decide el ganador | El contrato decide con una regla fija (más puntos; en empate, el primero en llegar) |
| Se puede pagar antes de tiempo o dos veces | Solo paga desde el día 10 del mes siguiente y una única vez |
| Nadie puede verificar el resultado | Puntos, ganador y pago quedan públicos en Stellar |
| Transferencia armada a mano | El dinero sale del contrato, ya fondeado por la comunidad |

## Para quién es

- **La administradora de la comunidad:** deja de calcular rankings y armar transferencias; solo aprueba.
- **Los embajadores:** saben que el premio se paga con las mismas reglas para todos y pueden verificarlo.

## Por qué elegí este problema

Es un problema real de la app [UpTeam](https://github.com/rbascunan/upteam), que gestiona embajadores
y StellarPoints. Es pequeño, se entiende en una frase y muestra lo que aporta la blockchain:
reglas que nadie puede saltarse y pagos que cualquiera puede verificar.

## Tips del bootcamp que tuvimos en cuenta

Entre otros consejos de las clases del bootcamp, estos guiaron el diseño:

**Producto** (Alberto Chávez, Trustless Work)
1. **¿Para quién construyes?** Para la administradora y los embajadores de una comunidad real. Cada función responde a una necesidad de uno de los dos.
2. **¿Qué no vas a construir? (Simplificar).** El contrato no calcula puntos por actividad ni maneja otros premios; solo resuelve el pago mensual obligatorio. Lo demás queda en la app UpTeam.
3. **¿Cómo lo vas a explicar? (Comunicar).** El problema se entiende en una frase y el contrato tiene solo 5 funciones con nombres en español.

**Smart contracts en Soroban** (Armando Cortés, Trustless Work)
1. **`require_auth()` de quien tiene el control.** Solo la firma de la administradora permite cargar puntos o aprobar; `pagar` no necesita firma porque las reglas del contrato deciden a quién y cuándo se paga.
2. **Código ordenado y seguro.** Todas las claves del storage están en un `enum DataKey`, los imports de `soroban_sdk` son explícitos (nunca `*`) y los errores tienen nombre propio en lugar de usar `unwrap()`.
3. **Probar a fondo, no solo el camino feliz.** Con `cargo test` se prueban los fallos esperados (sin aprobación, pago doble, firma faltante), se viaja en el tiempo con `env.ledger().set_timestamp(...)` para probar el día 10 y se lee el estado guardado después de cada llamada.

## Cómo funciona

| Función | Quién la usa | Qué hace |
|---|---|---|
| `iniciar(admin, token, monto)` | Una vez, al desplegar | Define la administradora, el token (USDC) y el monto del premio. |
| `cargar_puntos(periodo, embajador, puntos)` | Solo la administradora | Registra los puntos del mes (`periodo` = AAAAMM, ej. `202610`). El contrato recuerda al de más puntos. |
| `aprobar(periodo)` | Solo la administradora | Aprueba el premio. Después ya no se pueden cargar puntos de ese mes. |
| `pagar(periodo)` | Cualquiera | Transfiere el premio al ganador si está aprobado, ya es día 10 o más del mes siguiente y no se pagó antes. |
| `consultar(periodo)` | Cualquiera | Muestra ganador, puntos, si está aprobado, pagado y desde cuándo se puede pagar. |

Reglas:
- Gana quien tenga más puntos. **En empate gana el primero que llegó a ese puntaje.**
- Cada embajador se carga una sola vez por mes.
- El dinero sale del saldo del contrato: la comunidad lo fondea enviándole USDC.
- El día de pago se calcula dentro del contrato: para octubre 2026 (`202610`) es el 10/11/2026 00:00 UTC.

Flujo de un mes:

```
Durante el mes        Día 10 del mes siguiente
cargar_puntos  --->   aprobar (administradora)  --->  pagar  --->  50 USDC al ganador
```

## Cómo correrlo

Requisitos: [Rust](https://rustup.rs) y [Stellar CLI](https://developers.stellar.org/docs/tools/cli).

```bash
rustup target add wasm32v1-none

# Pruebas
cargo test

# Compilar el contrato
stellar contract build
# -> target/wasm32v1-none/release/premio_mensual.wasm
```

Desplegar en Testnet:

```bash
stellar keys generate admin --network testnet --fund

stellar contract deploy \
  --wasm target/wasm32v1-none/release/premio_mensual.wasm \
  --source admin --network testnet --alias premio_mensual

# Token USDC de prueba: usar la dirección del contrato del asset en Testnet
stellar contract invoke --id premio_mensual --source admin --network testnet -- \
  iniciar --admin admin --token <TOKEN_ID> --monto 500000000

stellar contract invoke --id premio_mensual --source admin --network testnet -- \
  cargar_puntos --periodo 202610 --embajador <DIRECCION> --puntos 1240

stellar contract invoke --id premio_mensual --source admin --network testnet -- \
  aprobar --periodo 202610

stellar contract invoke --id premio_mensual --source admin --network testnet -- \
  pagar --periodo 202610
```

## Pruebas

`cargo test` corre 8 pruebas en `contracts/premio-mensual/src/test.rs`:

```
test test::calcula_el_dia_10_del_mes_siguiente ... ok
test test::gana_el_de_mas_puntos_y_cobra_el_premio ... ok
test test::no_paga_sin_aprobacion_de_la_administradora ... ok
test test::no_paga_antes_del_dia_10 ... ok
test test::no_paga_dos_veces_el_mismo_mes ... ok
test test::en_empate_gana_el_primero_en_llegar ... ok
test test::no_se_puede_cargar_dos_veces_ni_despues_de_aprobar ... ok
test test::solo_la_administradora_puede_cargar_puntos ... ok

test result: ok. 8 passed; 0 failed
```

## Estructura

```
Cargo.toml
contracts/premio-mensual/
  Cargo.toml
  src/lib.rs    # contrato
  src/test.rs   # pruebas
```
