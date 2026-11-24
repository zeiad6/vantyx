xs = []
i = 0
while i < 150000:
    xs.append(i)
    i = i + 1
total = 0
i = 0
while i < len(xs):
    xs[i] = xs[i] + 1
    total = total + xs[i]
    i = i + 1
print(len(xs), total)
