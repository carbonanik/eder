"""Generate the original 10-second ambient score, accents, and closing chime."""
from array import array
from math import sin, cos, pi, exp
from pathlib import Path
import wave

RATE = 48000
DURATION = 10
samples = array('h')

def bell(t, start, frequency, level, decay):
    age = t - start
    if age < 0:
        return 0
    attack = min(1, age / .012)
    return level * attack * exp(-age / decay) * (
        sin(2*pi*frequency*age) + .18*sin(2*pi*2.003*frequency*age))

for i in range(RATE * DURATION):
    t = i / RATE
    fade = min(1, t / .8, max(0, (10 - t) / .65))
    left = right = 0.0
    for frequency, level in [(130.8128, .022), (195.9977, .018), (261.6256, .012), (329.6276, .009)]:
        envelope = .8 + .2 * sin(2*pi*.12*t)
        left += level * envelope * sin(2*pi*frequency*t)
        right += level * envelope * sin(2*pi*(frequency+.16)*t)
    accent = bell(t, 2.04, 523.251, .028, .35) + bell(t, 4.5, 659.255, .022, .3)
    chime = bell(t, 7.25, 523.251, .065, .8) + bell(t, 7.37, 783.991, .033, .7) + bell(t, 7.51, 1046.502, .02, .6)
    left = (left + accent + chime) * fade
    right = (right + accent + chime) * fade
    samples.extend((int(max(-1,min(1,left))*32767), int(max(-1,min(1,right))*32767)))

destination = Path(__file__).parent / 'public' / 'eder-score.wav'
with wave.open(str(destination), 'wb') as output:
    output.setnchannels(2)
    output.setsampwidth(2)
    output.setframerate(RATE)
    output.writeframes(samples.tobytes())
print(destination)
