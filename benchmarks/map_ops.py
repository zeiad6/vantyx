entries = {}
i = 0
while i < 3000:
    entries[i] = i * 2
    i = i + 1
total = 0
i = 0
while i < 3000:
    total = total + entries[i]
    i = i + 1
print(len(entries), total)
