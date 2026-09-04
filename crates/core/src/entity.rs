//! The simulation-side entity.
//!
//! One struct rather than an enum per kind. The fields map onto the wire field
//! groups in `schema`, so producing a snapshot is a copy and not a translation.
//! At a few hundred entities the wasted bytes on, say, a bullet's unused score are
//! not worth an enum's match arms in every physics loop.

use schema::{
    AgentId, Class, EntityId, EntityView, FocusId, HealthGroup, Kind, PhysicsGroup, PositionGroup,
    ScoreGroup, ShapeTier, Stats, StyleGroup, TeamGroup, TeamId, Tick, Vec2,
};

use crate::constants::{
    ALPHA_PEN_DRAG, BODY_DAMAGE_VS_PROJECTILE, BODY_DAMAGE_VS_SHAPE, BODY_DAMAGE_VS_TANK,
    BULLET_DAMAGE, BULLET_HP, BULLET_LIFETIME_TICKS, BULLET_RADIUS, CC_RADIUS, MASS_ALPHA,
    MASS_PENTAGON, MASS_SQUARE, MASS_TANK, MASS_TRIANGLE, SHAPE_BODY_DAMAGE_ALPHA,
    SHAPE_BODY_DAMAGE_PENTAGON, SHAPE_BODY_DAMAGE_SQUARE, SHAPE_BODY_DAMAGE_TRIANGLE, SHAPE_DRAG,
    SHAPE_HP_ALPHA, SHAPE_HP_PENTAGON, SHAPE_HP_SQUARE, SHAPE_HP_TRIANGLE, SHAPE_RADIUS_ALPHA,
    SHAPE_RADIUS_PENTAGON, SHAPE_RADIUS_SQUARE, SHAPE_RADIUS_TRIANGLE, SHAPE_VALUE_ALPHA,
    SHAPE_VALUE_PENTAGON, SHAPE_VALUE_SQUARE, SHAPE_VALUE_TRIANGLE, TANK_BASE_HP,
    TANK_HP_PER_LEVEL, TANK_RADIUS,
};

/// A live entity. Every kind shares this shape; `kind` decides which fields carry
/// meaning.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub kind: Kind,
    pub pos: Vec2,
    pub vel: Vec2,
    /// Radians. Zero points east, positive turns toward south.
    pub heading: f32,
    pub radius: f32,

    /// How hard this entity is to push. A contact splits its correction between
    /// the two bodies in inverse proportion to this, so the heavier one barely
    /// moves. See `constants::MASS_SQUARE` for how the hierarchy was derived.
    pub mass: f32,

    pub team: Option<TeamId>,
    /// The agent driving this entity. Tanks and control centers only.
    pub agent: Option<AgentId>,
    /// The entity that fired this bullet. Bullets only.
    pub owner: Option<EntityId>,

    pub hp: f32,
    pub max_hp: f32,
    pub last_damaged: Option<Tick>,

    pub score: u32,
    pub level: u8,

    pub tier: Option<ShapeTier>,
    /// Unused in v0. Every tank is identical.
    pub class: Option<Class>,
    pub stats: Stats,
    pub focus: Option<FocusId>,

    /// Tick this entity expires on. Bullets only.
    pub expires_at: Option<Tick>,
    /// Damage dealt on contact. A bullet's payload, a shape's or tank's body damage.
    pub contact_damage: f32,
    /// Last tick this tank fired. Drives the reload gate.
    pub last_fired: Option<Tick>,
    /// The last thing this bullet damaged.
    ///
    /// A bullet moves 8.8 units a tick and a tank is 20 across, so a bullet spends
    /// two or three ticks inside its target. Charging full damage on each of them
    /// would make a shot's damage a function of closing speed, and would put real
    /// damage per second at double the figure the regeneration threshold in
    /// `constants` is balanced against. One remembered target is enough: the
    /// failure it prevents is re-hitting the same entity on consecutive ticks.
    pub last_hit: Option<EntityId>,
}

impl Entity {
    fn blank(kind: Kind, pos: Vec2, radius: f32) -> Self {
        Self {
            kind,
            pos,
            vel: Vec2::ZERO,
            heading: 0.0,
            radius,
            mass: MASS_SQUARE,
            team: None,
            agent: None,
            owner: None,
            hp: 1.0,
            max_hp: 1.0,
            last_damaged: None,
            score: 0,
            level: 1,
            tier: None,
            class: None,
            stats: Stats::default(),
            focus: None,
            expires_at: None,
            contact_damage: 0.0,
            last_fired: None,
            last_hit: None,
        }
    }

    pub fn tank(agent: AgentId, team: TeamId, pos: Vec2, heading: f32) -> Self {
        let hp = tank_max_hp(1);
        Self {
            heading,
            team: Some(team),
            agent: Some(agent),
            hp,
            max_hp: hp,
            mass: MASS_TANK,
            contact_damage: BODY_DAMAGE_VS_TANK,
            ..Self::blank(Kind::Tank, pos, TANK_RADIUS)
        }
    }

    pub fn shape(tier: ShapeTier, pos: Vec2, vel: Vec2, focus: Option<FocusId>) -> Self {
        let radius = shape_radius(tier);
        let mass = shape_mass(tier);
        let (hp, damage) = match tier {
            ShapeTier::Square => (SHAPE_HP_SQUARE, SHAPE_BODY_DAMAGE_SQUARE),
            ShapeTier::Triangle => (SHAPE_HP_TRIANGLE, SHAPE_BODY_DAMAGE_TRIANGLE),
            ShapeTier::Pentagon => (SHAPE_HP_PENTAGON, SHAPE_BODY_DAMAGE_PENTAGON),
            ShapeTier::AlphaPentagon => (SHAPE_HP_ALPHA, SHAPE_BODY_DAMAGE_ALPHA),
        };
        Self {
            vel,
            mass,
            hp,
            max_hp: hp,
            tier: Some(tier),
            focus,
            contact_damage: damage,
            ..Self::blank(Kind::Shape, pos, radius)
        }
    }

    pub fn bullet(owner: EntityId, team: TeamId, pos: Vec2, vel: Vec2, now: Tick) -> Self {
        Self {
            vel,
            heading: vel.y.atan2(vel.x),
            team: Some(team),
            owner: Some(owner),
            hp: BULLET_HP,
            max_hp: BULLET_HP,
            expires_at: Some(Tick(now.0 + BULLET_LIFETIME_TICKS)),
            contact_damage: BULLET_DAMAGE,
            ..Self::blank(Kind::Bullet, pos, BULLET_RADIUS)
        }
    }

    /// A control center [CC]. Static, no collision, cannot be destroyed.
    pub fn control_center(agent: AgentId, team: TeamId, pos: Vec2) -> Self {
        Self {
            team: Some(team),
            agent: Some(agent),
            hp: f32::INFINITY,
            max_hp: f32::INFINITY,
            ..Self::blank(Kind::ControlCenter, pos, CC_RADIUS)
        }
    }

    pub fn is_tank(&self) -> bool {
        self.kind == Kind::Tank
    }

    pub fn is_bullet(&self) -> bool {
        self.kind == Kind::Bullet
    }

    pub fn is_shape(&self) -> bool {
        self.kind == Kind::Shape
    }

    /// A control center is a marker. It never moves, collides, or takes damage.
    pub fn is_inert(&self) -> bool {
        self.kind == Kind::ControlCenter
    }

    /// Body damage this entity deals to `other` on contact.
    ///
    /// The three multipliers come from `docs/STATS.md`: body damage is 50% higher
    /// against tanks and 75% lower against projectiles. A bullet carries its own
    /// payload instead and ignores the table.
    pub fn contact_damage_against(&self, other: &Entity) -> f32 {
        if self.is_bullet() {
            return self.contact_damage;
        }
        match other.kind {
            Kind::Bullet => BODY_DAMAGE_VS_PROJECTILE,
            Kind::Tank => BODY_DAMAGE_VS_TANK,
            Kind::Shape => {
                if self.is_shape() {
                    // Shapes drift through each other without damage. They are
                    // scenery to one another.
                    0.0
                } else {
                    BODY_DAMAGE_VS_SHAPE
                }
            }
            Kind::ControlCenter => 0.0,
        }
    }

    /// Points this entity is worth when destroyed. Read by the objective crate.
    pub fn value(&self) -> u32 {
        match (self.kind, self.tier) {
            (Kind::Shape, Some(ShapeTier::Square)) => SHAPE_VALUE_SQUARE,
            (Kind::Shape, Some(ShapeTier::Triangle)) => SHAPE_VALUE_TRIANGLE,
            (Kind::Shape, Some(ShapeTier::Pentagon)) => SHAPE_VALUE_PENTAGON,
            (Kind::Shape, Some(ShapeTier::AlphaPentagon)) => SHAPE_VALUE_ALPHA,
            // A tank's kill value is its accumulated score, so a fed tank is a
            // target worth coordinating on.
            (Kind::Tank, _) => self.score,
            _ => 0,
        }
    }

    pub fn to_view(&self, id: EntityId) -> EntityView {
        EntityView {
            id,
            position: PositionGroup {
                pos: self.pos,
                heading: self.heading,
            },
            physics: PhysicsGroup {
                vel: self.vel,
                radius: self.radius,
            },
            style: StyleGroup {
                kind: self.kind,
                tier: self.tier,
                class: self.class,
                stats: Some(self.stats),
                focus: self.focus,
            },
            team: TeamGroup {
                team: self.team,
                agent: self.agent,
                owner: self.owner,
            },
            health: if self.is_inert() {
                None
            } else {
                Some(HealthGroup {
                    hp: self.hp,
                    max_hp: self.max_hp,
                    last_damaged: self.last_damaged,
                })
            },
            score: if self.is_tank() {
                Some(ScoreGroup {
                    score: self.score,
                    level: self.level,
                })
            } else {
                None
            },
        }
    }
}

/// Tank health at a given level, before stat points. `docs/STATS.md`.
/// Mass of a shape of this tier.
///
/// Derived from the score table: a shape's mass is its point value divided by
/// [`SHAPE_VALUE_SQUARE`]. Written as constants rather than computed so that the
/// two can be tuned apart if a score change should not move the physics.
pub fn shape_mass(tier: ShapeTier) -> f32 {
    match tier {
        ShapeTier::Square => MASS_SQUARE,
        ShapeTier::Triangle => MASS_TRIANGLE,
        ShapeTier::Pentagon => MASS_PENTAGON,
        ShapeTier::AlphaPentagon => MASS_ALPHA,
    }
}

/// Velocity a shape of this tier retains per tick.
pub fn shape_drag(tier: ShapeTier) -> f32 {
    match tier {
        ShapeTier::AlphaPentagon => ALPHA_PEN_DRAG,
        _ => SHAPE_DRAG,
    }
}

/// Radius of a shape of this tier.
///
/// Exposed because the spawner needs the radius before the entity exists, in order
/// to inset a candidate point from the arena wall and to test it for overlap. One
/// table, read from both places.
pub fn shape_radius(tier: ShapeTier) -> f32 {
    match tier {
        ShapeTier::Square => SHAPE_RADIUS_SQUARE,
        ShapeTier::Triangle => SHAPE_RADIUS_TRIANGLE,
        ShapeTier::Pentagon => SHAPE_RADIUS_PENTAGON,
        ShapeTier::AlphaPentagon => SHAPE_RADIUS_ALPHA,
    }
}

pub fn tank_max_hp(level: u8) -> f32 {
    TANK_BASE_HP + TANK_HP_PER_LEVEL * (level.saturating_sub(1) as f32)
}
