text = "ab"
i = 0
while i < 15:
    text = text + text
    i = i + 1
count = 0
for c in text:
    if c == "a":
        count = count + 1
print(len(text), count)
