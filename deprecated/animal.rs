use fake::faker::name::raw::FirstName;
use fake::locales::EN;
use fake::Fake;
use rand::seq::SliceRandom;
use rand::Rng;

pub struct Animal;
pub struct PetName;

impl Animal {
    pub fn fake<R: Rng + ?Sized>(rng: &mut R) -> String {
        const ANIMALS: &[&str] = &[
            "Dog",
            "Cat",
            "Rabbit",
            "Hamster",
            "Guinea Pig",
            "Rat",
            "Mouse",
            "Gerbil",
            "Chinchilla",
            "Ferret",
            "Bird",
            "Parrot",
            "Canary",
            "Finch",
            "Fish",
            "Goldfish",
            "Turtle",
            "Snake",
            "Lizard",
            "Gecko",
        ];
        ANIMALS.choose(rng).unwrap().to_string()
    }
}

impl PetName {
    pub fn fake<R: Rng + ?Sized>(rng: &mut R) -> String {
        let name = FirstName(EN).fake::<String>();

        const PET_ADJECTIVES: &[&str] = &[
            "Fluffy", "Fuzzy", "Buddy", "Tiny", "Big", "Little", "Happy", "Sleepy", "Grumpy",
            "Snowy", "Whiskers", "Spots", "Cuddles",
        ];

        if rng.gen_bool(0.3) {
            format!("{} {}", name, PET_ADJECTIVES.choose(rng).unwrap())
        } else {
            name
        }
    }
}
