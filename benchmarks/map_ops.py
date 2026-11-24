keys = []
values = []
i = 0
while i < 3000:
    key = i
    found = -1
    j = 0
    while j < len(keys):
        if keys[j] == key:
            found = j
            break
        j = j + 1
    if found < 0:
        keys.append(key)
        values.append(key * 2)
    else:
        values[found] = key * 2
    i = i + 1
total = 0
i = 0
while i < 3000:
    key = i
    found = -1
    j = 0
    while j < len(keys):
        if keys[j] == key:
            found = j
            break
        j = j + 1
    total = total + values[found]
    i = i + 1
print(len(keys), total)
