# Playtest brief: the fun rework

Ten minutes, fresh creature. You judge whether Beastie is fun now; your verdict reopens the
[fun rework](fun-rework.md) if it isn't.

## Start

```sh
cargo xtask dev --fake-ai --new-game      # no model: Mop's own composed voice
./scripts/play.sh --new-game              # real local model (needs the model files)
```

Each new game is a new Mop, with its own favorite and disliked toy and food.

## What to try, and what should happen

1. **Meet Mop.** Click *Meet Mop*. Mop waits at the cave mouth, then swims up to the glass and
   looks at you. Click empty water: Mop looks where you tapped, ripples spread, and a curious Mop
   swims over.
2. **Play.** Click a toy in the rail or the tank. Mop perks up immediately and chases it: the ball
   gets nudged three more times, the bell rung, the sock towed around. A toy Mop dislikes gets a
   cross look and a bubble with that toy crossed out.
3. **Teach a word.** While Mop plays, a bubble shows the toy with a "?". Type its name (any word,
   even a made-up one). Mop tilts its head and tries to say it ("baw?"). Say it again: sparkles,
   a chime, and Mop says your word back. This should happen within a minute.
4. **Use the word.** Later, type the word on its own. Mop understands and goes for it (or, being
   Mop, sometimes refuses). While playing, Mop names things you taught it, unprompted.
5. **Feed.** Click a food in the rail; it drops in front of Mop and is eaten (or spat out) within a
   couple of seconds. Name the food while Mop eats. Mop will ask for a favorite by name when hungry.
6. **Its name.** Say "Mop" while it looks at you, a couple of times. Then calling it brings it over.
7. **Just watch.** Leave it alone for a minute. Mop should look busy and readable: going for a toy
   (dotted ring on the target), chasing a bubble, foraging, resting in the cave, glancing at you.
8. **Look closer.** Click Mop's name in the rail and choose *Inspect*: it lists the words you taught.

## Things to notice

- Did anything you did go unanswered, or need a second click?
- Was it ever unclear what Mop was doing or wanted?
- Did teaching make sense without these instructions?
- Did Mop surprise you? Did it feel like *your* Mop?
- Did any line feel generic, or was Mop repetitive?
- Was there a moment you wanted to stop?
