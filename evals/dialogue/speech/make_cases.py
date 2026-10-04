#!/usr/bin/env python3
"""Generate the frozen learned-word speech eval cases.

Each case is a DialogueRequest the session could emit: a speech intent, the creature's learned
vocabulary at some stage, and a mood. Cases are grouped by scenario; train and validation splits are
assigned by group so related variants never straddle the split. Re-running reproduces the same files.
"""
import json
import random

rng = random.Random(20261004)

TOY = lambda t: {"kind": "toy", "value": t}
FOOD = lambda f: {"kind": "food", "value": f}
ACT = lambda a: {"kind": "act", "value": a}
CREATURE = {"kind": "creature"}
PRAISE = {"kind": "praise"}
SCOLD = {"kind": "scold"}
GREETING = {"kind": "greeting"}

VOCABS = {
    "none": [],
    "first": [("ball", TOY("ball"))],
    "coined": [("zorp", TOY("ball")), ("blip", FOOD("berry"))],
    "few": [("ball", TOY("ball")), ("berry", FOOD("berry")), ("mop", CREATURE)],
    "some": [("ball", TOY("ball")), ("bell", TOY("bell")), ("berry", FOOD("berry")),
             ("mop", CREATURE), ("good", PRAISE)],
    "many": [("ball", TOY("ball")), ("bell", TOY("bell")), ("sock", TOY("sock")),
             ("berry", FOOD("berry")), ("shroom", FOOD("mushroom")), ("mop", CREATURE),
             ("good", PRAISE), ("no", SCOLD), ("hi", GREETING), ("play", ACT("play")),
             ("come", ACT("come"))],
}
LIMIT = {0: 3, 1: 3, 2: 3, 3: 5, 4: 5, 5: 5, 6: 5, 7: 5}

def limit(vocab):
    return LIMIT.get(len(vocab), 9)

SCENARIOS = []
def add(group, intent, vocab, mood="content", player_said=""):
    SCENARIOS.append((group, intent, vocab, mood, player_said))

for vocab in ["first", "few", "some", "many"]:
    word, meaning = VOCABS[vocab][0]
    add("new_word_" + vocab, {"kind": "new_word", "word": word, "meaning": meaning}, vocab, player_said=word)
add("new_word_coined", {"kind": "new_word", "word": "zorp", "meaning": TOY("ball")}, "coined", player_said="zorp")
add("new_word_name", {"kind": "new_word", "word": "mop", "meaning": CREATURE}, "few", player_said="mop")
for attempt, said in [("baw?", "ball"), ("so?", "sock"), ("zow?", "zorp"), ("mu?", "mushroom")]:
    add("echo_" + said, {"kind": "echo", "attempt": attempt}, "none", player_said=said)
    add("echo_" + said + "_later", {"kind": "echo", "attempt": attempt}, "some", player_said=said)
for vocab in ["first", "few", "some", "many"]:
    for response in ["comply", "refuse", "delight", "sulk", "look"]:
        word, meaning = VOCABS[vocab][0]
        if response in ("delight",) and vocab in ("few", "some", "many"):
            word, meaning = "mop", CREATURE
        if response == "sulk" and vocab in ("some", "many"):
            word, meaning = ("no", SCOLD) if vocab == "many" else ("good", PRAISE)
        add(f"answer_{response}_{vocab}", {"kind": "answer", "word": word, "meaning": meaning,
            "response": response}, vocab, mood="resentful" if response == "refuse" else "content",
            player_said=word)
for vocab, meaning, mood in [("few", FOOD("berry"), "hungry"), ("none", ACT("eat"), "hungry"),
                             ("some", TOY("bell"), "curious"), ("many", ACT("come"), "lonely"),
                             ("first", TOY("ball"), "curious"), ("coined", FOOD("berry"), "hungry")]:
    add(f"want_{vocab}_{meaning.get('value', meaning['kind'])}", {"kind": "want", "meaning": meaning}, vocab, mood=mood)
for vocab, meaning in [("first", TOY("ball")), ("some", FOOD("berry")), ("many", TOY("sock")),
                       ("coined", TOY("ball"))]:
    add(f"remark_{vocab}", {"kind": "remark", "meaning": meaning}, vocab)
for vocab in ["none", "first", "some"]:
    add("babble_" + vocab, {"kind": "babble"}, vocab, player_said="")

def request(index, scenario):
    group, intent, vocab_key, mood, said = scenario
    vocab = VOCABS[vocab_key]
    return {
        "protocol_version": 1, "request_id": 1000 + index, "creature_name": "Mop", "mood": mood,
        "known_concepts": [], "candidate_memories": [], "candidate_beliefs": [],
        "idiolect": {"quirk": "plain"}, "desired_social_act": None, "input_rejection": None,
        "context": {"version": 1, "recent_turns": [], "repetition_count": 0, "aquarium": None,
                    "relationship": None, "avoid_reply_fingerprints": [], "avoid_reply_texts": []},
        "interpretation": {"understood_concepts": [], "referenced_objects": [], "unknown_words": 0,
                           "ambiguous": False, "is_question": False},
        "player_said": said,
        "constraints": {"max_words": limit(vocab), "allowed_gestures": ["none", "look_player"]},
        "vocabulary": [{"word": w, "meaning": m} for w, m in vocab],
        "speech_intent": intent,
    }

groups = sorted({s[0] for s in SCENARIOS})
rng.shuffle(groups)
validation = set(groups[: len(groups) // 3])
with open("cases_train.jsonl", "w") as train, open("cases_val.jsonl", "w") as val:
    for index, scenario in enumerate(SCENARIOS):
        case = {"id": scenario[0], "request": request(index, scenario)}
        (val if scenario[0] in validation else train).write(json.dumps(case) + "\n")
print(len(SCENARIOS), "cases;", len(validation), "validation groups of", len(groups))
