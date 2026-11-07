#!/usr/bin/env python3
"""Seed a throwaway Axum API through its public endpoints with invented fitness data.

Volumes deliberately exceed the API's list pages: 620 Workouts (default page 100, maximum 500, with
same-day Workouts straddling page boundaries) and 540 Body Weight days. Refuses a non-loopback API so
it cannot write invented data into a real household server.
"""

import argparse
import datetime
import json
import sys
import urllib.parse
import urllib.request

WORKOUT_COUNT = 620
BODY_WEIGHT_DAYS = 540

# Added alongside the Library the API's migrations already seed.
ADDED_EXERCISES = [
    ("Fixture Pull-Up", "calisthenics", "back", None),
    ("Fixture Hold", "calisthenics", "shoulders", None),
    ("Fixture Squat", "gym", None, "barbell"),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://127.0.0.1:8000")
    parser.add_argument("--api-key", required=True)
    args = parser.parse_args()

    if urllib.parse.urlparse(args.base_url).hostname not in ("127.0.0.1", "localhost", "::1"):
        print("refusing to seed a non-loopback API", file=sys.stderr)
        return 2

    def send(method, path, payload=None):
        request = urllib.request.Request(
            args.base_url + path,
            method=method,
            data=None if payload is None else json.dumps(payload).encode(),
            headers={"Authorization": f"Bearer {args.api_key}", "Content-Type": "application/json"},
        )
        with urllib.request.urlopen(request) as response:
            body = response.read()
            return json.loads(body) if body else None

    if send("GET", "/api/workouts?limit=1") or send("GET", "/api/body_weight?limit=1"):
        print("refusing to seed an API that already has Workouts or Body Weight", file=sys.stderr)
        return 2

    ids = {
        name: send(
            "POST",
            "/api/exercises",
            {"name": name, "category": category, "muscle_group": muscle, "equipment": equipment},
        )["id"]
        for name, category, muscle, equipment in ADDED_EXERCISES
    }

    send(
        "PATCH",
        "/api/profile",
        {"height_cm": 178.5, "sex": "male", "hr_max_bpm": 192, "birth_date": "1994-03-17"},
    )

    start = datetime.date(2024, 1, 1)
    for day in range(BODY_WEIGHT_DAYS):
        date = (start + datetime.timedelta(days=day)).isoformat()
        send("POST", "/api/body_weight", {"date": date, "weight_kg": round(80 + (day % 40) / 10, 1)})

    # Every tenth Workout shares its date with the previous one, so page boundaries fall inside a day.
    day = 0
    for number in range(1, WORKOUT_COUNT + 1):
        if number % 10 != 0:
            day += 1
        date = (start + datetime.timedelta(days=day)).isoformat()
        send(
            "POST",
            "/api/workouts",
            {
                "date": date,
                "name": f"Workout {number:03d}" if number % 7 != 0 else None,
                "notes": f"Notes for Workout {number:03d}" if number % 5 == 0 else None,
                "exercises": [
                    {
                        "exercise_id": ids["Fixture Pull-Up"],
                        "order_index": 0,
                        "notes": "Strict form" if number % 3 == 0 else None,
                        "sets": [
                            {"set_number": 1, "reps": 8, "weight_kg": None, "rpe": 7},
                            {"set_number": 2, "reps": 6, "weight_kg": 12.5, "rpe": 9},
                        ],
                    },
                    {
                        "exercise_id": ids["Fixture Hold"],
                        "order_index": 1,
                        "sets": [{"set_number": 1, "duration_seconds": 20 + number % 30}],
                    },
                    {
                        "exercise_id": ids["Fixture Squat"],
                        "order_index": 2,
                        "sets": [{"set_number": 1, "reps": 5, "weight_kg": 100, "rpe": 8, "notes": "Belt"}],
                    },
                ],
            },
        )

    print(f"Seeded {WORKOUT_COUNT} Workouts, {BODY_WEIGHT_DAYS} Body Weight days, {len(ids)} added Exercises")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
