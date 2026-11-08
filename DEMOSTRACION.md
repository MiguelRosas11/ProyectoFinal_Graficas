# Demostración de Aster // Deriva

## Recorrido de 27 segundos

| Tiempo | Qué se ve |
| --- | --- |
| 00:00–00:02 | Vuelo espacial con la nave, el Sol y varios planetas en el mismo encuadre |
| 00:02–00:08 | Acercamiento a Edén. La distancia radial baja de 31.1 a 2.7 unidades |
| 00:08–00:22 | Una vuelta completa. Árboles y faros permanecen orientados sobre la superficie mientras Edén gira |
| 00:22–00:27 | Alejamiento de Edén hasta regresar a espacio abierto |

El MP4 muestra el recorrido guiado. Para demostrar los controles en vivo, arranca el programa y usa el vuelo manual descrito abajo.

## Vuelo manual

1. Pulsa **T**, mantén **W** durante unos dos segundos y suelta. Pulsa **F** al entrar en el rango local. Pulsa **T** otra vez si quieres centrar Edén.
2. Mantén **W** hasta observar los árboles de cerca. Si sigues acercándote, el aviso `LIMITE DE SUPERFICIE` indica que actuó la colisión.
3. Mantén **D** para rodear Edén. Mueve las flechas para mirar alrededor y usa **T** para centrarlo otra vez. Mantén **S** para alejarte.
4. Pulsa **Tab** y **T** para apuntar a otro planeta. Usa **W** y **Shift** para recorrer el tramo en espacio abierto y suelta el impulso antes de entrar en modo local.
5. Pulsa **F1** para abrir la telemetría. Compara `TRI ENVIADOS` al alternar **C** y al alternar **L**. Con detalle automático, acerca y aleja la nave para ver cambiar los niveles bajo, medio y alto.

## Criterios solicitados para evaluación

| Bloque | Características y evidencia |
| --- | --- |
| Planetas | Cinco cuerpos con órbita y rotación. Selección con **Tab**. Las siluetas, las superficies, los objetos o los anillos permiten distinguirlos. |
| Superficie | Árbol, faro, cristal y roca propios, construidos en `mesh.rs`. Las instancias se distribuyen mediante ruido fractal con semilla configurable en `world.rs`. El eje vertical de cada objeto se alinea con la dirección radial y su base acompaña el giro del planeta. |
| Shaders | Sol con plasma animado, bandas y tormentas de Vesper, reflejos de Nival con normal map y costas y agua animada de Lumen. Edén y Ferrum también tienen sombreado propio. |
| Geometría | La subdivisión de `icosphere` crea triángulos a partir del icosaedro antes de renderizar. La función `terrain` modifica el radio después de subdividir. Ferrum tiene un cráter amplio que altera la silueta al rodearlo. |
| Navegación | Vuelo libre y modo local, aceleración y giro amortiguados, freno, atracción de los planetas y colisión con el terreno. **G** permite comparar la deriva con y sin gravedad. |
| Rendimiento | Descarte por esfera contra el frustum y selección de tres mallas según el tamaño aparente. **C** y **L** cambian el trabajo que llega al rasterizador. **F1** muestra los contadores. |
| Presentación | Tablero de vuelo, indicador de objetivo, guía integrada y cinco identidades visuales dentro de la misma escena. |

En la vista inicial del recorrido y con la semilla de entrega, la prueba de comparación registró **22,012** triángulos enviados con la configuración automática, **26,862** sin descarte y **40,252** con detalle alto fijo y descarte activo. La prueba también comprueba que desactivar el descarte conserva el mismo framebuffer.

```powershell
cargo test --locked --release culling_preserves_framebuffer_and_lod_reduces_submissions -- --nocapture
```

## Cambios que se pueden explicar en vivo

- En `src/world.rs`, cambia `period` de Edén. Al reducirlo, aumenta su velocidad angular alrededor del Sol. `spin` cambia su rotación propia de forma independiente.
- Ejecuta con `--seed 1122` y vuelve a Edén. Cambian el relieve y la distribución. Repite con la misma semilla y se reproducen las mismas posiciones.
- En `src/shading.rs`, cambia la velocidad de desplazamiento del ruido en `Shader::Sun`. El plasma se mueve más rápido sin afectar la órbita de los planetas.
- En `src/render.rs`, sigue un vértice desde su transformación hasta el píxel. Explica por qué se interpola `1/z`, qué ocurre al cruzar el plano cercano y cuándo se ejecuta el shader de fragmentos.
