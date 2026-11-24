class Point:
    __slots__ = ("x", "y")

    def __init__(self, x, y):
        self.x = x
        self.y = y

points = []
i = 0
while i < 80000:
    points.append(Point(i, i + 1))
    i = i + 1
total = 0
i = 0
while i < len(points):
    points[i].x = points[i].x + 1
    total = total + points[i].x + points[i].y
    i = i + 1
print(len(points), total)
