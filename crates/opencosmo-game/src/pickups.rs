//! What each collectible is worth, and what picking it up does.
//!
//! The original decides this in the player-contact sprite `switch`
//! (game1.c:7473-7570), keyed by *sprite* type rather than actor type -
//! several actor types share one sprite, so keying this table the same way
//! keeps it one entry per distinct pickup.
//!
//! Replaces an earlier hand-written list of "things that look collectible",
//! which was missing most of the food and every gem - the reason so little
//! could actually be picked up.

/// Score values are the original's own, and vary far more than the flat
/// 100 this used to award.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pickup {
    /// Plain score.
    Score(u32),
    /// Counted in the status bar's "Stars" field rather than scored
    /// (game1.c:7391).
    Star,
    /// Stocks the bomb counter, and scores 100 (game1.c:7561-7566).
    Bomb,
    /// Widens the health meter by one cell, up to 5 (game1.c:7543).
    Hamburger,
    /// Heals if hurt, otherwise pays out big (game1.c:7474-7488).
    PowerUp,
    /// Wraps the player in a bubble that makes them untouchable for a
    /// while (game1.c:7775-7782). The hint globes call this "this shield
    /// for temporary invincibility".
    ///
    /// Awards no score, faithfully: the original spawns a 12800 score
    /// *effect* over it and never adds the points - a bug in the shipped
    /// game, flagged as such in cosmore's own comment.
    Invincibility,
}

/// `SPR_*` id -> what collecting it does. `None` means "not a pickup".
pub fn pickup_for_sprite(spr: u16) -> Option<Pickup> {
    Some(match spr {
        1 => Pickup::Star,                    // SPR_STAR
        28 => Pickup::PowerUp,                // SPR_POWER_UP
        57 => Pickup::Bomb,                   // SPR_BOMB_IDLE
        82 => Pickup::Hamburger,              // SPR_HAMBURGER
        189 => Pickup::Invincibility,         // SPR_INVINCIBILITY_CUBE
        // Tomatoes, pear, onion.
        32 | 34 | 36 | 38 => Pickup::Score(200),
        // The bulk of the food.
        94 | 134 | 135 | 136 | 137 | 138 | 139 | 140 | 147 | 168 | 170 | 172 | 226 | 229
        | 232 => Pickup::Score(400),
        // The four the original singles out for double, plus the gems that
        // share their value (game1.c:7523-7527, 7600-7606).
        85 | 141 | 146 | 223 | 194 | 196 | 198 | 200 | 220 => Pickup::Score(800),
        // Crystals.
        154 | 155 => Pickup::Score(1600),
        // Ornament, emerald, diamond.
        153 | 174 | 176 => Pickup::Score(3200),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invincibility_cube_is_collectible() {
        // It rendered and animated but had no pickup entry, so it could not
        // be touched at all - and it is the shield hint 3 talks about.
        assert_eq!(pickup_for_sprite(189), Some(Pickup::Invincibility));
    }

    #[test]
    fn covers_the_pickups_that_were_previously_missed() {
        // Every one of these rendered but could not be collected before.
        for spr in [36, 38, 85, 94, 134, 146, 223, 229, 232, 220] {
            assert!(
                pickup_for_sprite(spr).is_some(),
                "sprite {spr} should be collectible"
            );
        }
    }

    #[test]
    fn distinguishes_the_special_counters() {
        assert_eq!(pickup_for_sprite(1), Some(Pickup::Star));
        assert_eq!(pickup_for_sprite(57), Some(Pickup::Bomb));
        assert_eq!(pickup_for_sprite(82), Some(Pickup::Hamburger));
        assert_eq!(pickup_for_sprite(28), Some(Pickup::PowerUp));
    }

    #[test]
    fn scores_vary_by_item() {
        assert_eq!(pickup_for_sprite(32), Some(Pickup::Score(200)));
        assert_eq!(pickup_for_sprite(135), Some(Pickup::Score(400)));
        assert_eq!(pickup_for_sprite(141), Some(Pickup::Score(800)));
        assert_eq!(pickup_for_sprite(154), Some(Pickup::Score(1600)));
        assert_eq!(pickup_for_sprite(176), Some(Pickup::Score(3200)));
    }

    #[test]
    fn scenery_is_not_a_pickup() {
        assert_eq!(pickup_for_sprite(39), None); // SPR_EXIT_SIGN
        assert_eq!(pickup_for_sprite(26), None); // SPR_EXPLOSION
    }
}
