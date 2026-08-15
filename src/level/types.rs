/// Meia-largura da arena em unidades de mundo.
pub const ARENA_HALF_W: f32 = 640.0;
/// Meia-altura visivel.
pub const ARENA_HALF_H: f32 = 240.0;

/// Uma peca de geometria, como dado.
///
/// Fases descrevem, nao constroem: um `&'static [Piece]` pode ser conferido por
/// teste -- se um patamar ficar fora do alcance do pulo, o mapa nasce com um
/// jogador preso e nada no jogo reclama.
#[derive(Debug, Clone, Copy)]
pub enum Piece {
    /// Bloco macico. `top` e o meio da superficie de cima, nao o centro.
    Terrain {
        /// Meio da superficie superior.
        top: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Altura em celulas.
        rows: u16,
    },
    /// Teto: bloco macico pendurado, medido pela face de baixo.
    ///
    /// Fisicamente e igual a [`Piece::Terrain`] -- a fisica ja para quem sobe
    /// contra um solido. O que muda e que ele nao conta como apoio: o topo de
    /// um teto nao e lugar de ficar de pe, e cobrar que alguem chegue la
    /// reprovaria um mapa correto.
    Ceiling {
        /// Meio da face inferior.
        bottom: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Espessura em celulas.
        rows: u16,
    },
    /// Plataforma fina, atravessavel por baixo.
    Platform {
        /// Meio da plataforma.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
    },
    /// Corrente escalavel pendurada a partir de `top`.
    Chain {
        /// Onde ela e presa.
        top: Vec2,
        /// Quantidade de elos.
        links: u16,
    },
    /// Superficie perigosa. Nao e solida: normalmente fica sobre um piso ou
    /// no fundo de um poco e machuca quem encostar.
    Hazard {
        /// Meio da faixa perigosa.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Material, que decide arte, dano e empurrao.
        kind: HazardKind,
    },
    /// Fonte que jorra de tempos em tempos.
    ///
    /// A diferenca para [`Piece::Hazard`] nao e de grau: uma poca so cobra
    /// atencao uma vez -- o jogador aprende onde ela esta e nunca mais pisa
    /// ali. Uma fonte cobra atencao o round inteiro, porque o lugar seguro
    /// deixa de ser um lugar e passa a ser um lugar *e uma hora*.
    Geyser {
        /// Base da coluna, na altura do chao de onde ela sai.
        at: Vec2,
        /// Largura da boca, em celulas.
        cols: u16,
        /// Altura do jorro cheio, em celulas.
        rows: u16,
        /// Segundos entre um jorro e o proximo.
        period: f32,
        /// Deslocamento no ciclo. Duas fontes em fase jorram juntas e viram
        /// uma parede; fora de fase, elas fazem o jogador escolher.
        phase: f32,
        kind: HazardKind,
    },
    /// Poca que sobe e desce, engolindo o que estiver baixo demais.
    ///
    /// E o oposto da fonte: em vez de o perigo vir buscar, o chao seguro
    /// afunda. Uma plataforma que so vale metade do tempo vale mais que duas
    /// que valem sempre.
    Tide {
        /// Superficie na mare baixa.
        at: Vec2,
        /// Largura em celulas.
        cols: u16,
        /// Quanto ela sobe, em celulas.
        rise: u16,
        /// Segundos de um ciclo completo, subida e descida.
        period: f32,
        phase: f32,
        kind: HazardKind,
    },
    /// Goteira: uma boca no alto que pinga material corrosivo.
    Drip {
        /// Onde a boca fica.
        from: Vec2,
        /// Largura da boca, para as gotas nao cairem sempre na mesma coluna.
        cols: u16,
        /// Altura em que a gota se desmancha.
        floor: f32,
        /// Segundos entre uma gota e a proxima.
        period: f32,
        phase: f32,
        kind: HazardKind,
    },
}

/// Perigos reutilizados pelas arenas tematicas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardKind {
    Lava,
    Acid,
    Spikes,
    /// Fogo de jade: o material do jardim, que o dragao cospe.
    Jade,
}

impl HazardKind {
    fn damage(self) -> i32 {
        match self {
            Self::Lava => 34,
            Self::Acid => 18,
            Self::Spikes => 24,
            Self::Jade => 22,
        }
    }

    /// Para onde ele joga quem encosta.
    ///
    /// Lava explode para cima; acido so corroi e empurra de leve; espinho
    /// devolve; jade queima e levanta. O `x` sempre inverte o proprio impulso
    /// do jogador -- quem entrou correndo sai voltando.
    fn knockback(self, velocity: Vec2) -> Vec2 {
        match self {
            Self::Lava => Vec2::new(velocity.x * -0.35, 430.0),
            Self::Acid => Vec2::new(velocity.x * -0.2, 210.0),
            Self::Spikes => Vec2::new(velocity.x * -0.4, 300.0),
            Self::Jade => Vec2::new(velocity.x * -0.3, 380.0),
        }
    }

    /// Cor do respingo que ele solta.
    fn spray(self) -> Color {
        match self {
            Self::Lava => palette::MAGMA,
            Self::Acid => palette::TOXIC,
            Self::Spikes => palette::ASH,
            Self::Jade => palette::JADE,
        }
    }
}

impl Piece {
    /// Faixa horizontal e altura da superficie em que da para ficar de pe, se
    /// esta peca tiver uma. Corrente nao tem.
    ///
    /// Usado pelos testes de geometria e por quem precisa saber onde alguem
    /// vai parar sem simular a queda.
    pub fn foothold(self) -> Option<(f32, f32, f32)> {
        match self {
            Piece::Terrain { top, cols, .. } => {
                let half = cols as f32 * CELL.x * 0.5;
                Some((top.x - half, top.x + half, top.y))
            }
            Piece::Platform { at, cols } => {
                let half = cols as f32 * CELL.x * 0.5;
                Some((at.x - half, at.x + half, at.y + CELL.y * 0.5))
            }
            Piece::Chain { .. }
            | Piece::Ceiling { .. }
            | Piece::Hazard { .. }
            | Piece::Geyser { .. }
            | Piece::Tide { .. }
            | Piece::Drip { .. } => None,
        }
    }

    /// Faixa horizontal que esta peca torna perigosa, se tornar.
    ///
    /// Um ponto de nascimento dentro dela e um jogador que perde vida antes de
    /// encostar no chao -- e a fonte e a mare tem o agravante de nao estarem
    /// visiveis no instante em que alguem escolhe onde por o spawn.
    ///
    /// So os testes perguntam: em jogo, quem machuca e a entidade ja montada.
    /// Aqui a pergunta e sobre o dado, antes de existir entidade nenhuma.
    #[cfg(test)]
    pub fn menace(self) -> Option<(f32, f32)> {
        let span = |at: Vec2, cols: u16| {
            let half = cols as f32 * CELL.x * 0.5;
            Some((at.x - half, at.x + half))
        };
        match self {
            Piece::Hazard { at, cols, .. }
            | Piece::Geyser { at, cols, .. }
            | Piece::Tide { at, cols, .. } => span(at, cols),
            Piece::Drip { from, cols, .. } => span(from, cols),
            _ => None,
        }
    }
}

/// Contrato de uma fase.
///
/// Tudo aqui e dado. Nenhuma fase toca em `Commands`: quem monta e
/// [`build_level`], e por isso um mapa novo nao pode inventar uma regra de
/// spawn diferente das outras.
pub trait Level: Send + Sync + 'static {
    /// Nome exibido na tela de controles.
    fn name(&self) -> &'static str;
    /// Onde cada jogador nasce, por indice.
    fn spawn_points(&self) -> &'static [Vec2];
    /// Pontos de onde armas sao largadas.
    fn drop_points(&self) -> &'static [Vec2];
    /// Geometria jogavel.
    fn pieces(&self) -> &'static [Piece];
    /// Predios do fundo.
    fn skyline(&self) -> &'static [Building];
    /// Letreiros de neon.
    fn signs(&self) -> &'static [Sign];
    /// Fundo tematico da arena.
    fn theme(&self) -> Theme {
        self.scene().theme()
    }
    /// Pintura concreta do fundo; mapas do mesmo tema nao compartilham cartao.
    fn scene(&self) -> Scene {
        Scene::City
    }

    /// Topo do apoio mais alto que fica abaixo de `from`, na coluna dela.
    ///
    /// E onde alguem largado nesse ponto vai parar. Um ponto de spawn nao e o
    /// chao -- e so de onde os jogadores comecam a cair -- entao quem precisa
    /// posicionar algo em pe sem simular a queda pergunta aqui.
    fn ground_under(&self, from: Vec2) -> Option<f32> {
        self.pieces()
            .iter()
            .filter_map(|piece| piece.foothold())
            .filter(|(x0, x1, y)| from.x >= *x0 && from.x <= *x1 && *y <= from.y)
            .map(|(_, _, y)| y)
            .max_by(f32::total_cmp)
    }
}

/// Fase atualmente carregada.
#[derive(Resource)]
pub struct CurrentLevel(pub Box<dyn Level>);

/// Marca tudo que pertence a geometria da fase, para limpar no restart.
#[derive(Component)]
pub struct LevelGeometry;

/// Corrente escalavel: sobrepor e segurar cima faz o boneco subir.
#[derive(Component)]
pub struct Climbable;

/// Um elo da corrente Verlet. Elos consecutivos mantem distancia fixa; se um
/// deles for destruido, a parte abaixo perde a conexao e cai naturalmente.
#[derive(Component)]
pub struct ChainParticle {
    chain: u8,
    index: u16,
    pub(crate) previous: Vec2,
    pinned: bool,
}

