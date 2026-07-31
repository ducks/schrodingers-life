use serde::Serialize;

const SPECIES: &[&str] = &["cat", "duck", "ghost", "blob", "owl", "robot"];
const EYES: &[&str] = &["·", "✦", "×", "◉", "@", "°"];
const HATS: &[&str] = &["crown", "tophat", "propeller", "halo", "wizard", "beanie"];

#[derive(Debug, Clone, Serialize)]
pub struct Creature {
    pub species: String,
    pub eye: String,
    pub hat: Option<String>,
    pub rarity: Rarity,
    pub shiny: bool,
    pub frames: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Uncommon => "uncommon",
            Self::Rare => "rare",
            Self::Epic => "epic",
            Self::Legendary => "legendary",
        }
    }
}

pub fn roll(seed: &str) -> Creature {
    let mut random = mulberry32(fnv1a(&format!("{seed}:schrodingers-life:v1")));
    let species = pick(&mut random, SPECIES).to_owned();
    let eye = pick(&mut random, EYES).to_owned();
    let rarity = roll_rarity(&mut random);
    let hat = (!matches!(rarity, Rarity::Common)).then(|| pick(&mut random, HATS).to_owned());
    let shiny = random() < 0.01;
    let frames = frames(&species, &eye);

    Creature {
        species,
        eye,
        hat,
        rarity,
        shiny,
        frames,
    }
}

fn frames(species: &str, eye: &str) -> Vec<Vec<String>> {
    let raw = match species {
        "cat" => vec![
            vec![
                "   /\\_/\\   ",
                "  ( {E} {E} )  ",
                "  (  w  )  ",
                "  (\")_(\")  ",
            ],
            vec![
                "   /\\_/\\   ",
                "  ( {E} {E} )  ",
                "  (  w  )  ",
                "  (\")_(\")~ ",
            ],
            vec![
                "   /\\-/\\   ",
                "  ( {E} {E} )  ",
                "  (  w  )  ",
                "  (\")_(\")  ",
            ],
        ],
        "duck" => vec![
            vec!["    __     ", "  <({E} )___ ", "   (  ._>  ", "    `--'   "],
            vec!["    __     ", "  <({E} )___ ", "   (  ._>  ", "    `--'~  "],
            vec!["    __     ", "  <({E} )___ ", "   (  .__> ", "    `--'   "],
        ],
        "ghost" => vec![
            vec![
                "   .----.  ",
                "  / {E}  {E} \\ ",
                "  |      | ",
                "  ~`~``~`~ ",
            ],
            vec![
                "   .----.  ",
                "  / {E}  {E} \\ ",
                "  |      | ",
                "  `~`~~`~` ",
            ],
            vec![
                "   .----.  ",
                "  / {E}  {E} \\ ",
                "  |      | ",
                "  ~~`~~`~~ ",
            ],
        ],
        "owl" => vec![
            vec![
                "   /\\  /\\  ",
                "  (({E})({E})) ",
                "  (  ><  ) ",
                "   `----'  ",
            ],
            vec![
                "   /\\  /\\  ",
                "  (({E})({E})) ",
                "  (  ><  ) ",
                "   .----.  ",
            ],
            vec![
                "   /\\  /\\  ",
                "  (({E})(-)) ",
                "  (  ><  ) ",
                "   `----'  ",
            ],
        ],
        "robot" => vec![
            vec![
                "   .[||].  ",
                "  [ {E}  {E} ] ",
                "  [ ==== ] ",
                "  `------' ",
            ],
            vec![
                "   .[||].  ",
                "  [ {E}  {E} ] ",
                "  [ -==- ] ",
                "  `------' ",
            ],
            vec![
                "   .[||].  ",
                "  [ {E}  {E} ] ",
                "  [ ==== ] ",
                "  `------' ",
            ],
        ],
        _ => vec![
            vec![
                "   .----.  ",
                "  ( {E}  {E} ) ",
                "  (      ) ",
                "   `----'  ",
            ],
            vec![
                "  .------. ",
                " (  {E}  {E}  )",
                " (        )",
                "  `------' ",
            ],
            vec![
                "    .--.   ",
                "   ({E}  {E})  ",
                "   (    )  ",
                "    `--'   ",
            ],
        ],
    };

    raw.into_iter()
        .map(|frame| {
            frame
                .into_iter()
                .map(|line| line.replace("{E}", eye))
                .collect()
        })
        .collect()
}

fn roll_rarity(random: &mut impl FnMut() -> f64) -> Rarity {
    match (random() * 100.0) as u32 {
        0 => Rarity::Legendary,
        1..=4 => Rarity::Epic,
        5..=14 => Rarity::Rare,
        15..=39 => Rarity::Uncommon,
        _ => Rarity::Common,
    }
}

fn pick<'a>(random: &mut impl FnMut() -> f64, values: &'a [&str]) -> &'a str {
    values[(random() * values.len() as f64) as usize]
}

fn fnv1a(value: &str) -> u32 {
    value.bytes().fold(2_166_136_261, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    })
}

fn mulberry32(mut seed: u32) -> impl FnMut() -> f64 {
    move || {
        seed = seed.wrapping_add(0x6d2b79f5);
        let mut value = seed;
        value = (value ^ (value >> 15)).wrapping_mul(1 | value);
        value = value.wrapping_add((value ^ (value >> 7)).wrapping_mul(61 | value)) ^ value;
        f64::from(value ^ (value >> 14)) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_are_deterministic() {
        let first = serde_json::to_string(&roll("life-1")).unwrap();
        let second = serde_json::to_string(&roll("life-1")).unwrap();
        assert_eq!(first, second);
    }
}
