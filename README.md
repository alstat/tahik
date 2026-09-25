# Tahik

A 3D settlement-builder about the Sama Dilaut (Bajau), the sea nomads of the
southern Philippines. It borrows ideas from Manor Lords, but your land is the
*tahik*, the sea.

Sail a banka on the monsoon winds and cast nets over the deep. Dive for *tayum*
(sea urchins) on the reefs, and raise stilt villages on the shallow turquoise
shoals. Link the homes with walkways, catch rain to drink, farm fish and
seaweed, and sell your goods to the land folk at Bongao market. Watch out for
sharks in deep water, *ribut* storms in the wet season, and the long thirst of
the dry season.

**Goal:** build 4 thriving villages (3+ families and 50%+ approval each) on
different shoals.

## Running

```sh
cargo run --release
```

There's no music, only nature sounds (waves, wind, rain, thunder, splashes and
bubbles). The game synthesises all of them at startup, so it needs no asset
files.

## How to play

You're the overseer. Watch over your families from above and give them work.
The families then sail off and do it on their own.

### Overseer view (the default)

| Input | Action |
| --- | --- |
| WASD / arrow keys | Pan across the sea |
| Q / E, right-drag | Rotate and tilt the camera |
| Mouse wheel | Zoom |
| **Build bar** (bottom center) | Pick a category (Homes, Food, Materials, Water), then a building card. Left-click (or press Enter) on the water where the ghost turns green. B opens the bar; 1-5 pick a card. |
| Click a family (on the water or in the list on the left) | Select it. Its task buttons appear; press 1-7 to choose. |
| T | Game speed ×1 / ×2 / ×4 |
| H | Help |
| Esc | Close the build bar / deselect the family |
| Tab | Captain view: steer your own boat |

### Families and work

- You start with **two families** living on their boats at Sitangkai.
- **Work areas** (placed from the build bar) staff themselves with any family on
  *Rest*:
  - **Fishing ground** (Food): goes in deep water. The family fishes there,
    brings fish home while the village needs food, and sells the rest at Bongao.
  - **Tayum dive site** (Food): goes on a reef flat. The family dives for sea
    urchins and sells them.
  - **Woodcutter camp** and **Nipa gatherers** (Materials): go on Bongao's
    shore. The family cuts timber or gathers coconut and nipa leaves for the
    village stockpile.
- **Tasks** you can give directly:
  - *Build & repair:* build blueprints using the stockpile, buying any missing
    timber and nipa at Bongao; fix storm damage.
  - *Fish for the village*, *Fish to sell* and *Dive for tayum*.
  - *Fetch water:* buy water at Bongao and fill the village jars.
  - *Trade goods:* carry dried fish and seaweed to market.
  - *Rest & glean:* gather a little food near home.
- **Buildings** are blueprints that builders construct over time. Houses need
  a timber frame and a nipa roof.
- **Prayer:** five times a day (Fajr, Dhuhr, Asr, Maghrib, Isha) the azan
  sounds. Families stop work and gather at the langgal, or pray at home or on
  their boats if they're far away. Villagers walk the walkways to the langgal.
  The built-in azan is a synthesized, distant chant. Put a real recording at
  `assets/azan.ogg` and the game will play that instead.
- **Moving in and out:** new families sail in when a village has empty houses
  and 50%+ approval. Very unhappy villages lose families, but never their last.

### Captain view (Tab)

W/S paddle, A/D steer, Space raises/lowers the sail, F casts a net, E dives
for tayum, G trades at a village or at Bongao, R repairs.

## Developer flags

These environment variables are for testing:

- `TAHIK_AUTOSTART`: skip the title screen
- `TAHIK_DEMO`: start with a small village already built at Sitangkai
- `TAHIK_TIME=0.75`: start at a given time of day (0 to 1)
- `TAHIK_STORM`: start with a storm raging
- `TAHIK_SAIL`: start with the sail up
- `TAHIK_AT=x,z`: start the boat at a world position
- `TAHIK_KEYS=3:B,4.5:Enter`: simulate key taps at the given seconds
- `TAHIK_TASKS=1,5`: give the starting families tasks 1-7
- `TAHIK_BLUEPRINT`: place a stilt house blueprint at Sitangkai
- `TAHIK_WORKSITES`: place a fishing ground, a woodcutter camp and nipa gatherers
- `TAHIK_CAT=1`: open a build-bar category
- `TAHIK_SPEED=4`: start at a game speed
- `TAHIK_CAM=x,z,dist,pitch,yaw`: point the overseer camera
- `TAHIK_ECHO`: print game log messages to the terminal
- `TAHIK_SHOT=out.png` and `TAHIK_SHOT_AT=6`: save a screenshot after that many seconds, then quit
