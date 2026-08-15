/// A composicao parada do tema.
///
/// De tras para frente, e tudo apoiado em [`GROUND`]: cenario nasce no chao da
/// arena, nao numa altura escrita a mao que ninguem sabe conferir.
fn panels(scene: Scene) -> Vec<Panel> {
    match scene {
        Scene::City => vec![
            // Morros baixos atras da cidade: eles so aparecem nos vaos entre um
            // predio e outro, e e isso que da fundo ao horizonte.
            Panel::footed(
                ridge(&CITY_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            Panel::new(
                AsciiArt::solid(MOON, palette::ASH),
                Vec2::new(-420.0, 150.0),
                SKY,
            ),
            Panel::footed(
                AsciiArt::solid(MAST, palette::IRON),
                Vec2::new(470.0, GROUND + 230.0),
                FAR,
            ),
            Panel::footed(
                AsciiArt::solid(TANK, palette::IRON),
                Vec2::new(-250.0, GROUND + 240.0),
                FAR,
            ),
            // A moldura da arena: uma viga em cima e dois pilares nas pontas.
            // Ela nao e paisagem, e a borda do ringue -- por isso fica no plano
            // do jogo e nao desliza com o resto.
            Panel::new(
                AsciiArt::fill('═', SPAN, 1, palette::IRON),
                Vec2::new(0.0, 205.0),
                WORLD,
            ),
            Panel::new(
                AsciiArt::fill('║', 1, 25, palette::IRON),
                Vec2::new(-590.0, 5.0),
                WORLD,
            ),
            Panel::new(
                AsciiArt::fill('║', 1, 25, palette::IRON),
                Vec2::new(590.0, 5.0),
                WORLD,
            ),
        ],
        // A cratera nao esta aqui: quem a desenha e a propria boca, em
        // `vents`, porque a cor dela e o aviso de que a montanha vai estourar.
        Scene::Caldera => vec![
            Panel::new(
                smog(SPAN, 5, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 24.0),
                SKY,
            ),
            Panel::footed(
                ridge(&VOLCANO_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // A borda de dentro da caldeira, entre a serra e o cone: sem esta
            // faixa morna a montanha ficava recortada contra o nada.
            Panel::footed(
                ridge(&CALDERA_RIM, SPAN, palette::SCENE_CINDER),
                Vec2::new(0.0, GROUND + 16.0),
                FAR,
            ),
            Panel::footed(cone(palette::IRON, palette::SCENE_FIRE), VOLCANO_FOOT, MID),
            // Chao derretido correndo na linha do horizonte, na frente da
            // montanha: e o que deixa claro que a arena inteira esta dentro da
            // caldeira, e nao so olhando para ela de longe.
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
            // Lascas de obsidiana emoldurando o poco. Precisam passar da
            // altura da serra para existirem: uma lasca da altura dela e uma
            // peca inteira que se dissolve no fundo sem ninguem notar. A da
            // direita vai espelhada -- o gerador e determinista, e duas copias
            // identicas nas duas pontas da tela leem como moldura de cartaz.
            Panel::footed(basalt(10, 13, &STONE), Vec2::new(-575.0, GROUND), NEAR),
            Panel::footed(
                basalt(10, 13, &STONE).mirrored(),
                Vec2::new(575.0, GROUND),
                NEAR,
            ),
        ],
        Scene::MagmaBridge => vec![
            Panel::new(
                smog(SPAN, 4, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 22.0),
                SKY,
            ),
            Panel::footed(
                ridge(&CHASM_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // As duas paredes do desfiladeiro, subindo alem do topo da tela.
            // Elas nao sao paisagem: sao o motivo de a ponte existir.
            Panel::footed(basalt(26, 26, &STONE), Vec2::new(-536.0, GROUND), FAR),
            Panel::footed(
                basalt(26, 26, &STONE).mirrored(),
                Vec2::new(536.0, GROUND),
                FAR,
            ),
            // A travessia que ja caiu, pendurada atras da que ainda esta em pe.
            Panel::new(suspension(60, 12, &STEEL), Vec2::new(0.0, 58.0), MID),
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
        ],
        Scene::ForgeCore => vec![
            Panel::new(
                smog(SPAN, 6, palette::COAL),
                Vec2::new(0.0, ARENA_HALF_H - 28.0),
                SKY,
            ),
            Panel::footed(
                gantry(12, palette::COAL),
                Vec2::new(0.0, GROUND + 250.0),
                FAR,
            ),
            Panel::footed(furnace(30, 13, &FURNACE), Vec2::new(-330.0, GROUND), MID),
            // Bigorna e trilhos ficam parados; o malho que desce entre eles e
            // uma entidade a parte, em `shows`.
            Panel::footed(
                AsciiArt::tinted(ANVIL, &STEEL, palette::IRON),
                Vec2::new(HAMMER_AT.x, GROUND),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 15, palette::IRON),
                Vec2::new(HAMMER_AT.x - 56.0, GROUND + 90.0),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 15, palette::IRON),
                Vec2::new(HAMMER_AT.x + 56.0, GROUND + 90.0),
                MID,
            ),
            Panel::footed(
                current(SPAN, 2, 0, &MAGMA_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 24, palette::IRON),
                Vec2::new(-470.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                AsciiArt::fill('║', 1, 24, palette::IRON),
                Vec2::new(470.0, GROUND),
                NEAR,
            ),
        ],
        Scene::AcidWorks => vec![
            // Cinza esverdeada em vez de cinza: o ceu da fabrica ja e o aviso.
            Panel::new(
                smog(SPAN, 4, palette::SCENE_TOXIC),
                Vec2::new(0.0, ARENA_HALF_H - 20.0),
                SKY,
            ),
            Panel::footed(
                ridge(&STACK_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // Coluna de destilacao: a peca mais alta do patio.
            Panel::footed(vat(9, 18, &TANK_SKIN), Vec2::new(-470.0, VAT_FOOT), MID),
            Panel::footed(
                vat(VAT_COLS, STACKS[0].1, &TANK_SKIN),
                Vec2::new(STACKS[0].0, VAT_FOOT),
                MID,
            ),
            Panel::footed(
                vat(VAT_COLS, STACKS[1].1, &TANK_SKIN),
                Vec2::new(STACKS[1].0, VAT_FOOT),
                MID,
            ),
            Panel::footed(
                vat(VAT_COLS, STACKS[2].1, &TANK_SKIN),
                Vec2::new(STACKS[2].0, VAT_FOOT),
                MID,
            ),
            // Contra o ceu, e nao contra a serra: o vao da placa e
            // transparente, e sobre a montanha o texto se perde na pedra.
            Panel::new(
                AsciiArt::solid(PLACARD, palette::SCENE_TOXIC),
                Vec2::new(-330.0, 44.0),
                MID,
            ),
            Panel::new(
                AsciiArt::solid(PLACARD, palette::SCENE_RUST),
                Vec2::new(360.0, 44.0),
                MID,
            ),
            // Encanamento por cima de tudo, na frente: da teto ao patio sem
            // fechar a leitura da briga.
            Panel::footed(pipeline(SPAN, &STEEL), Vec2::new(0.0, 122.0), NEAR),
        ],
        Scene::Reactor => vec![
            Panel::footed(
                gantry(12, palette::IRON),
                Vec2::new(0.0, GROUND + 250.0),
                FAR,
            ),
            // Anel de contencao vazado: o nucleo que mora no meio dele pulsa
            // sozinho, em `flows`, e por isso nao entra na composicao parada.
            Panel::new(disc(13, true, &CONTAINMENT), CORE_AT, MID),
            // Os tirantes tem que encostar no anel. Curtos, eles viram dois
            // tracos boiando ao lado dele.
            Panel::footed(
                AsciiArt::fill('═', 34, 1, palette::IRON),
                Vec2::new(-250.0, CORE_AT.y),
                MID,
            ),
            Panel::footed(
                AsciiArt::fill('═', 34, 1, palette::IRON),
                Vec2::new(250.0, CORE_AT.y),
                MID,
            ),
            Panel::footed(drains(SPAN, palette::COAL), Vec2::new(0.0, GROUND), NEAR),
        ],
        Scene::Drainage => vec![
            // Fechado por cima: o unico mapa do jogo que acontece embaixo da
            // terra tem que ter teto, senao ele e um patio escuro qualquer.
            Panel::new(vault(SPAN, 5, &STONE), Vec2::new(0.0, 200.0), SKY),
            // Galeria funda, atras da principal: e ela que da corredor ao
            // subsolo em vez de uma parede de arcos.
            Panel::footed(
                arcade(3, 66, 11, &STONE),
                Vec2::new(0.0, GROUND + 40.0),
                FAR,
            ),
            // Galeria de arcos de verdade: a curva e meia-circunferencia, e e
            // ela que separa subsolo de porta quadrada.
            Panel::footed(arcade(5, 40, 16, &STONE), Vec2::new(0.0, GROUND), MID),
            Panel::footed(
                current(SPAN, 2, 0, &ACID_FLOW),
                Vec2::new(0.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                pipeline(SPAN, &STEEL),
                Vec2::new(0.0, ARENA_HALF_H - 84.0),
                NEAR,
            ),
        ],
        Scene::RedGate => vec![
            // Bruma alta: o portao precisa de um ceu com materia, senao ele
            // fica recortado contra o vazio como um adesivo.
            Panel::new(
                smog(SPAN, 3, palette::SCENE_HAZE),
                Vec2::new(0.0, ARENA_HALF_H - 18.0),
                SKY,
            ),
            Panel::footed(
                ridge(&GATE_RIDGE, SPAN, palette::SCENE_HAZE),
                Vec2::new(0.0, GROUND),
                SKY,
            ),
            // Escadaria e portao no mesmo eixo: o portao pousa no ultimo
            // degrau em vez de flutuar sobre ele.
            Panel::footed(terrace(4, 44, 5, &STONE), Vec2::new(0.0, GROUND), FAR),
            Panel::footed(gate(52, 14, &LACQUER), Vec2::new(0.0, GROUND + 46.0), MID),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(-360.0, GROUND),
                NEAR,
            ),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(360.0, GROUND),
                NEAR,
            ),
            Panel::new(
                AsciiArt::solid(BRANCH, palette::COAL),
                Vec2::new(-450.0, 185.0),
                NEAR,
            ),
        ],
        Scene::SunsetPagoda => vec![
            Panel::new(disc(14, false, &SUNSET), Vec2::new(-330.0, 84.0), SKY),
            Panel::new(
                smog(SPAN, 3, palette::SCENE_HAZE),
                Vec2::new(0.0, ARENA_HALF_H - 16.0),
                FAR,
            ),
            Panel::footed(
                ridge(&EAST_RIDGE, SPAN, palette::SCENE_HAZE),
                Vec2::new(0.0, GROUND),
                FAR,
            ),
            // O pagode pousa no ultimo degrau do terraco, e nao dentro dele:
            // duas pecas no mesmo plano que se cruzam disputam quem cobre
            // quem, e o que aparece vira sorteio da ordem de spawn.
            Panel::footed(terrace(3, 30, 5, &STONE), Vec2::new(270.0, GROUND), MID),
            Panel::footed(pagoda(5, &TIMBER), Vec2::new(270.0, GROUND + 48.0), MID),
        ],
        Scene::DragonGarden => vec![
            Panel::new(disc(12, true, &LUNAR), Vec2::new(-430.0, 150.0), SKY),
            Panel::footed(
                ridge(&GARDEN_RIDGE, SPAN, palette::COAL),
                Vec2::new(0.0, GROUND),
                FAR,
            ),
            // O dragao nao esta aqui: ele nao e composicao parada, e bicho.
            // Nasce em [`hatch_dragon`], anda pelo proprio rastro e volta para
            // o oito sozinho.
            Panel::footed(jade_pillar(13, &JADE_SKIN), Vec2::new(-556.0, GROUND), NEAR),
            Panel::footed(jade_pillar(13, &JADE_SKIN), Vec2::new(556.0, GROUND), NEAR),
            Panel::footed(bamboo(6, 11, &JADE_SKIN), Vec2::new(-390.0, GROUND), NEAR),
            Panel::footed(
                AsciiArt::tinted(STONE_LANTERN, &STONE, palette::IRON),
                Vec2::new(430.0, GROUND),
                NEAR,
            ),
        ],
    }
}

// --- pecas que se mexem sozinhas --------------------------------------------

