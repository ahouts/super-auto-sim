use crate::{
    dice::Dice,
    food::Food,
    friend::Friend,
    modifier::Modifier,
    params::TEAM_SIZE,
    params::{DEFAULT_GOLD, SHOP_ANIMAL_COUNT, SHOP_FOOD_COUNT},
    species::Species,
    team::Team,
};
use log::trace;

#[derive(Copy, Clone, Debug)]
enum ShopAction {
    BuyFriend,
    BuyCombineFriend,
    SellFriend,
    BuyFood,
    CombineFriends,
    Reroll,
}

impl ShopAction {
    pub fn sample<R: Dice>(rng: &mut R) -> Self {
        match rng.roll(0..6) {
            0 => Self::BuyFriend,
            1 => Self::BuyCombineFriend,
            2 => Self::SellFriend,
            3 => Self::BuyFood,
            4 => Self::CombineFriends,
            5 => Self::Reroll,
            i => panic!("Invalid ShopAction {}", i),
        }
    }
}

#[derive(Copy, Clone, Hash, Debug, Eq, PartialEq)]
pub struct Shop {
    pub team: Team,
    pub gold: usize,

    // XXX More slots get added to the shop over time
    shop_friends: [Option<Friend>; SHOP_ANIMAL_COUNT],
    shop_foods: [Option<Food>; SHOP_FOOD_COUNT],
}

impl Shop {
    pub fn new<R: Dice>(rng: &mut R) -> Self {
        let mut out = Shop {
            team: Team::new(),
            gold: DEFAULT_GOLD,
            shop_friends: [None; SHOP_ANIMAL_COUNT],
            shop_foods: [None; SHOP_FOOD_COUNT],
        };
        out.reroll(rng);
        out
    }

    /// Rerolls the shop
    fn reroll<R: Dice>(&mut self, rng: &mut R) {
        for a in self.shop_friends.iter_mut() {
            *a = Some(Friend::new(Species::sample(rng)));
        }

        for f in self.shop_foods.iter_mut() {
            *f = Some(Food::sample(rng));
        }
        self.shop_friends.sort();
        self.shop_foods.sort();
    }

    /// Picks a random friend from the shop, returning its index
    pub fn random_friend<R: Dice>(&self, rng: &mut R) -> Option<usize> {
        crate::dice::pick_one(rng, &self.shop_friends)
    }

    /// Picks a random food from the shop, returning its index
    pub fn random_food<R: Dice>(&self, rng: &mut R) -> Option<usize> {
        crate::dice::pick_one(rng, &self.shop_foods)
    }

    /// Buys the friend at `shop_pos` and adds it to `team_pos`
    fn buy_friend<R: Dice>(
        &mut self,
        shop_pos: usize,
        team_pos: usize,
        rng: &mut R,
    ) {
        assert!(self.gold >= 3);
        assert!(self.team[team_pos].is_none());

        self.gold -= 3;
        let friend = self.shop_friends[shop_pos].take().unwrap();
        self.shop_friends.sort();

        trace!("Buying {} at position {}", friend.species, team_pos);
        self.on_buy(friend, rng);
        self.team.summon(friend, team_pos);
        // XXX: There are also "friend is bought" triggers, but nothing in Tier
        // 1 uses them
    }

    fn combine_friends(&mut self, team_pos: usize, g: Friend) {
        let f = self.team[team_pos].as_mut().unwrap();
        assert!(f.species == g.species);
        trace!("Combining {} at position {}", f.species, team_pos);
        f.health = std::cmp::max(f.health, g.health) + 1;
        f.attack = std::cmp::max(f.attack, g.attack) + 1;
        f.exp += 1;
        // TODO: handle level-up here
    }

    fn sell_friend<R: Dice>(&mut self, team_pos: usize, rng: &mut R) {
        assert!(self.team[team_pos].is_some());

        let a = self.team[team_pos].take().unwrap();
        trace!("Selling {} at position {}", a.species, team_pos);

        self.gold += a.level();
        self.on_sell(a, rng);
        for i in 0..TEAM_SIZE {
            if i != team_pos && self.team[i].is_some() {
                self.on_sold(i);
            }
        }
    }

    /// Purchases food at the given shop position, applying it to the chosen
    /// member of the team.
    fn buy_food(&mut self, shop_pos: usize, team_pos: usize) {
        assert!(self.shop_foods[shop_pos].is_some());
        assert!(self.team[team_pos].is_some());

        let friend = self.team[team_pos].as_mut().unwrap();
        let food = self.shop_foods[shop_pos].take().unwrap();
        self.shop_foods.sort();

        self.gold -= 3;
        trace!(
            "Buying {} for {} at position {}",
            food,
            friend.species,
            team_pos
        );
        match food {
            Food::Apple => {
                trace!("    Buffing by ❤️  +1, ⚔️  +1");
                friend.attack += 1;
                friend.health += 1;
            }
            Food::Honey => {
                trace!("    Applying honey modifier");
                friend.modifier = Some(Modifier::Honey);
            }
        }
    }

    /// Asks an species to perform its on-buy action.  It has not yet been added
    /// to the team at this point.
    fn on_buy<R: Dice>(&mut self, f: Friend, rng: &mut R) {
        match f.species {
            Species::Otter => {
                // Give a random friend (+1, +1)
                for i in self.team.random_friends(1, rng) {
                    let g = self.team[i].as_mut().unwrap();
                    trace!(
                        "    {} on buy bufs {} at {} by ❤️  +1, ⚔️  +1",
                        f.species,
                        g.species,
                        i
                    );
                    g.health += 1;
                    g.attack += 1;
                }
            }
            _ => (),
        }
    }

    /// Asks an species to perform its on-sell action.  It has been removed
    /// from the team at this point.
    fn on_sell<R: Dice>(&mut self, a: Friend, rng: &mut R) {
        match a.species {
            Species::Beaver => {
                // Give two random friends +1 Health
                let delta = a.level();
                for i in self.team.random_friends(2, rng) {
                    let f = self.team[i].as_mut().unwrap();
                    trace!(
                        "    {} on sell bufs {} at {} b❤️  +{} ",
                        a.species,
                        f.species,
                        i,
                        delta
                    );
                    f.health += delta;
                }
            }
            Species::Duck => {
                // Give shop pets bonus health depending on level
                let delta = a.level();
                for f in self.shop_friends.iter_mut().flatten() {
                    trace!(
                        "    {} on sell bufs {} in shop by ❤️  +{}",
                        a.species,
                        f.species,
                        delta
                    );
                    f.health += delta;
                }
            }
            Species::Pig => {
                let delta = a.level();
                trace!("    {} on sell gives 🪙 +{}", a.species, delta);
                self.gold += delta;
            }
            _ => (),
        }
    }

    fn on_sold(&mut self, _i: usize) {
        // No Tier 1 friends have an on-sold trigger
    }

    pub fn step<R: Dice + std::fmt::Debug>(&mut self, rng: &mut R) -> bool {
        match ShopAction::sample(rng) {
            // Buy an species
            ShopAction::BuyFriend => {
                if self.gold < 3 {
                    trace!("Not enough gold to buy a friend; exiting");
                    return true;
                }
                if let Some(i) = self.random_friend(rng) {
                    let a = self.shop_friends[i].unwrap().species;
                    let j = rng.roll(0..TEAM_SIZE);
                    if self.team.make_space_at(j) {
                        self.buy_friend(i, j, rng);
                    } else {
                        trace!("Can't make space for {}; exiting", a);
                        return true;
                    }
                } else {
                    trace!("No friends in the shop; exiting");
                    return true;
                }
            }
            // Buy food
            ShopAction::BuyFood => {
                if self.gold < 3 {
                    trace!("Not enough gold to buy food; exiting");
                    return true;
                }
                let i = match self.random_food(rng) {
                    Some(i) => i,
                    None => {
                        trace!("No food in the shop; exiting");
                        return true;
                    }
                };
                let j = match self.team.random_friend(rng) {
                    Some(j) => j,
                    None => {
                        trace!("No friends to feed; exiting");
                        return true;
                    }
                };
                self.buy_food(i, j);
            }
            // Sell friend
            ShopAction::SellFriend => {
                if let Some(j) = self.team.random_friend(rng) {
                    self.sell_friend(j, rng);
                } else {
                    trace!("No friends to sell; exiting");
                    return true;
                }
            }
            // Reroll
            ShopAction::Reroll => {
                // We only reroll shops if they are missing animals or food
                // _or_ have any animals with non-default power.  If there are
                // animals with default power, then we
                // could have _different_ animals in a different timeline,
                // so rerolling doesn't accomplish anything.
                if self.gold == 0 {
                    trace!("No gold to reroll; exiting");
                    return true;
                } else if self.shop_foods.iter().any(Option::is_none)
                    || self.shop_friends.iter().any(Option::is_none)
                {
                    trace!("Re-rolling shop");
                    self.reroll(rng);
                    self.gold -= 1;
                } else {
                    trace!("Rerolling shop doesn't accomplish anything");
                    return true;
                }
            }
            // Attempt to combine
            ShopAction::CombineFriends => {
                let mut has_targets = [false; TEAM_SIZE];
                let mut targets = [[false; TEAM_SIZE]; TEAM_SIZE];
                for i in 0..TEAM_SIZE {
                    for j in (i + 1)..TEAM_SIZE {
                        let a = self.team[i];
                        let b = self.team[j];
                        if a.is_some()
                            && b.is_some()
                            && a.unwrap().species == b.unwrap().species
                        {
                            targets[i][j] = true;
                            targets[j][i] = true;
                            has_targets[i] = true;
                            has_targets[j] = true;
                        }
                    }
                }
                let num = has_targets.iter().filter(|i| **i).count();
                let i = has_targets
                    .iter()
                    .enumerate()
                    .filter(|i| *i.1)
                    .nth(rng.roll(0..num));

                if let Some((i, b)) = i {
                    assert!(b);
                    let num = targets[i].iter().filter(|j| **j).count();
                    let (j, b) = targets[i]
                        .iter()
                        .enumerate()
                        .filter(|j| *j.1)
                        .nth(rng.roll(0..num))
                        .unwrap();

                    assert!(b);
                    let friend = self.team[i].take().unwrap();
                    trace!("Merging {} at {} into {}", friend.species, i, j);
                    self.combine_friends(j, friend);
                } else {
                    trace!("No friends to combine; exiting");
                    return true;
                }
            }
            ShopAction::BuyCombineFriend => {
                if self.gold < 3 {
                    trace!(
                        "Not enough gold to buy and combine friend; exiting"
                    );
                    return true;
                }
                let mut has_targets = [false; SHOP_ANIMAL_COUNT];
                let mut targets = [[false; TEAM_SIZE]; SHOP_ANIMAL_COUNT];
                for i in 0..SHOP_ANIMAL_COUNT {
                    for j in 0..TEAM_SIZE {
                        let a = self.shop_friends[i];
                        let b = self.team[j];
                        if a.is_some()
                            && b.is_some()
                            && a.unwrap().species == b.unwrap().species
                        {
                            targets[i][j] = true;
                            has_targets[i] = true;
                        }
                    }
                }
                let num = has_targets.iter().filter(|i| **i).count();
                let i = has_targets
                    .iter()
                    .enumerate()
                    .filter(|i| *i.1)
                    .nth(rng.roll(0..num));

                let i = match i {
                    None => {
                        trace!("No friends to combine; exiting");
                        return true;
                    }
                    Some((i, b)) => {
                        assert!(b);
                        i
                    }
                };
                let num = targets[i].iter().filter(|j| **j).count();
                let (j, b) = targets[i]
                    .iter()
                    .enumerate()
                    .filter(|j| *j.1)
                    .nth(rng.roll(0..num))
                    .unwrap();

                assert!(b);
                let friend = self.shop_friends[i].take().unwrap();
                self.shop_friends.sort();

                self.gold -= 3;
                self.combine_friends(j, friend);
                trace!("Buying {} and combining at {}", friend.species, j);

                // The on-buy trigger happens after the friends are
                // combined, which matters in cases where the species
                // levels up.  For convenience, we remove the species from
                // the team briefly, then reinstall it.
                let friend = self.team[j].take().unwrap();
                self.on_buy(friend, rng);
                self.team[j] = Some(friend);

                // XXX: There are also "friend is bought" triggers, but
                // nothing in Tier 1 uses them
            }
        }
        false
    }
}
