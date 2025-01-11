# Adhan V2

## Objective: Split adhan into client-server model

Server:

- Generate Timetable
- Emit events via REST API

Client:

- Read events from server
- Call Adhan based on event

## REST API

- / - Index page
- /hadith - Hadith of the day
- /salah/today - Times for today (all times require current location)
- /salah/current - Redirects to /salah/today
- /salah/now - Redirects to /salah/today
- /salah/day/<day> - Times for day in current month (1-31) (Requires validation for current month when day > 28)
- /salah/month - Times for current month
- /salah/month/<month_int> - Times for month (int between 1 - 12) (Rank 0)
- /salah/month/<month_str> - Times for month (short named month) (Rank 1)
- /salah/month/<long_month_str> - Times for month (long named month) (Rank 2)
- /salah/year - Times for current year
