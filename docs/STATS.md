# Diep.io Stats Reference

## For Agents:
Source: [diepio.fandom.com/wiki/Stats](https://diepio.fandom.com/wiki/Stats)

Ignore adjustments made for various types of tanks, and focus on the stats themselves if the tanks have not been made already

Stats let a player customize a tank beyond its class. Skill Points (SP) unlock this customization as the tank levels up.

## Skill Point Mechanics

- 1 Skill Point (SP) per level up to Level 28.
- 1 SP at Level 30.
- 1 SP every 3 levels from Level 30 to Level 45.
- Maximum SP: 33.
- Each stat caps at 7 upgrades, except the Smasher branch (Smasher, Landmine, Auto Smasher, Spike), which caps at 10.
- Once placed, an SP cannot normally be removed. Exceptions: switching to a bulletless class (Smasher, Landmine, Spike) refunds all Bullet Speed/Penetration/Damage/Reload points; switching away from Auto Smasher refunds any 8th–10th points; switching to a Dominator (Sandbox only) refunds every point.

## Core Stats

### Health Regen

Restores lost Health Points (HP) after a period without damage. Regen rate jumps sharply ("Hyper Regen") after about 30 seconds undamaged.

Formula: `Durability Regen Per Second = 1/30 * Durability * (0.03 + 0.12 * Regen Stat)` — each point adds roughly +12% faster regen.

| Points | Time to Full Health (Lv.1 tank) | Avg. Regen Rate |
|---|---|---|
| 0 | 31.97s | 3.12%/s |
| 1 | 30.67s | 3.26%/s |
| 2 | 23.07s | 4.33%/s |
| 3 | 15.15s | 6.60%/s |
| 4 | 11.75s | 8.51%/s |
| 5 | 9.13s | 10.95%/s |
| 6 | 7.72s | 12.95%/s |
| 7 | 6.41s | 15.60%/s |

9+ points (Smasher branch only) restores full health inside the 30-second pre-Hyper-Regen window.

### Max Health

Base HP = 50 + [2 × (Level − 1)]. A Level 1 tank starts at 50 HP; a Level 45 tank starts at 138 HP before upgrades.

| Points | HP Increase |
|---|---|
| 0 | +0 |
| 1 | +20 |
| 2 | +40 |
| 3 | +60 |
| 4 | +80 |
| 5 | +100 |
| 6 | +120 |
| 7 | +140 |
| 8*| +160 |
| 9*| +180 |
| 10*| +200 |

\* Smasher branch only.

### Body Damage

Damage dealt on collision. Formula: `(Points + 5) × multiplier`. Player tanks use multiplier 4 vs. shapes. Body Damage is +50% vs. tanks and −75% vs. projectiles.

| Points | vs. Shape | vs. Tank | vs. Projectile |
|---|---|---|---|
| 0 | 20 | 30 | 5 |
| 1 | 24 | 36 | 6 |
| 2 | 28 | 42 | 7 |
| 3 | 32 | 48 | 8 |
| 4 | 36 | 54 | 9 |
| 5 | 40 | 60 | 10 |
| 6 | 44 | 66 | 11 |
| 7 | 48 | 72 | 12 |
| 8* | 52 | 78 | 13 |
| 9* | 56 | 84 | 14 |
| 10* | 60 | 90 | 15 |

\* Smasher branch only. The Spike has a base Body Damage +2 above other classes (its own point scale is shifted by −2 in the table above).

At max Body Damage, a tank kills a Pentagon in three taps.

### Bullet Speed

Controls projectile velocity and range. Becomes Drone Speed on Overseer-branch classes. Unavailable to Smasher, Spike, and Landmine (no bullets).

### Bullet Penetration

Effectively bullet HP — how many hits/objects a bullet survives before breaking. Each point adds 75% of base bullet HP. Becomes Drone Health on Overseer-branch classes. Unavailable to Smasher, Spike, Landmine.

| Points | Bullet HP | Bullet HP ×4 (Basic Tank) |
|---|---|---|
| 0 | 2 | 8 |
| 1 | 3.5 | 14 |
| 2 | 5 | 20 |
| 3 | 6.5 | 26 |
| 4 | 8 | 32 |
| 5 | 9.5 | 38 |
| 6 | 11 | 44 |
| 7 | 12.5 | 50 |
| 8* | 14 | 56 |
| 9* | 15.5 | 62 |
| 10* | 17 | 68 |

\* Auto Smasher only.

### Bullet Damage

Damage per bullet hit. Each point adds ~42.857% of base damage. −75% effect vs. other bullets/traps. Becomes Drone Damage on Overseer-branch classes. Unavailable to Smasher, Spike, Landmine.

| Points | vs. Tank/Shape/Drone | vs. Bullet/Trap |
|---|---|---|
| 0 | 7 | 1.75 |
| 1 | 10 | 2.5 |
| 2 | 13 | 3.25 |
| 3 | 16 | 4 |
| 4 | 19 | 4.75 |
| 5 | 22 | 5.5 |
| 6 | 25 | 6.25 |
| 7 | 28 | 7 |
| 8* | 31 | 7.75 |
| 9* | 34 | 8.5 |
| 10* | 37 | 9.25 |

\* Auto Smasher only (base bullet is ~70% weaker than Basic Tank's regardless).

### Reload

Fires bullets faster. Basic Tank base rate: 100 bullets/minute (0.6s/bullet at 0 points). Becomes Drone spawn rate on Drone-branch classes, and Drone Count on Necromancer. Unavailable to Smasher, Spike, Landmine.

| Points | Seconds/Bullet | Ticks/Bullet (1 tick = 0.04s) |
|---|---|---|
| 0 | 0.60s | 15t |
| 1 | 0.56s | 14t |
| 2 | 0.52s | 13t |
| 3 | 0.48s | 12t |
| 4 | 0.44s | 11t |
| 5 | 0.40s | 10t |
| 6 | 0.36s | 9t |
| 7 | 0.32s | 8t |

### Movement Speed

Base tank speed, excluding recoil effects. Decreases passively as the tank levels up. Smasher branch gets 10 points instead of 7.

## Hidden Stats

Not directly upgradable — driven by class choice.

### Bullet Accuracy / Bullet Spread

Accuracy = how straight a barrel fires; Spread = the firing cone angle. Streamliner and Ranger have high accuracy (low spread); Sprayer and Machine Gun have low accuracy (high spread).

### Recoil

Backward push on firing. Annihilator has the highest recoil per shot (6.8 background squares). Snipers, Spread Shot, and Gunner have negligible recoil. Flank Guard variants with symmetric cannon layouts (except Tri-Angle and Auto 3/Auto 5) cancel their own recoil out.

### Knockback Resistance

Reduces displacement from collisions. Only marginally affected by Body Damage or Movement Speed investment. Motherships and Dominators have ~100% resistance (Sandbox only).

### Field of View (FoV)

Camera size; scales slightly with level, more sharply with class.

| Tier | FoV | Classes |
|---|---|---|
| 1 | 142.8% | Ranger |
| 2 | 125% | Assassin, Stalker |
| 3 | 117.6% | Hunter, Streamliner, Predator |
| 4 | 111.1% | Sniper, Overseer branch, Trapper branch, Smasher branch, Skimmer, Rocketeer, Glider |
| 5 | 100% | All other tanks |

## Achievements Tied to Stats

- **2fast4u** — max Movement Speed
- **Ratatatatatatatata** — max Reload
- **More dangerous than it looks** — max Bullet Damage
- **Mach 4** — max Bullet Speed
- **There's no stopping it!** — max Bullet Penetration
- **Don't touch me** — max Body Damage
- **Self-repairing** — max Health Regen
- **Indestructible** — max Max Health

## Trivia

- Health Regen did not exist at Diep.io's April 15th launch.
- A Level 1 tank's base HP is exactly 50.0.
- Smasher and Landmine briefly showed "Upgrade to Windows 10" in place of unavailable bullet stats, before this was removed in the July 31st update.
- Auto Smasher (chosen at Level 45) is the only class with all 8 stats available at a 10-point cap.

## Known Bugs (as of page capture)

- **Patched:** leveling up used to instantly heal the player to full health.
- **Unpatched:** in Sandbox, queuing bullet stats then switching to Smasher and back via the class-switch key can desync the stat counter.
- **Unpatched:** maxing Reload at Level 45 can occasionally stick the cannon in its firing animation, most notably on the Annihilator.
