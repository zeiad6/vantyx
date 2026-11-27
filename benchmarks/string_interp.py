def main() -> None:
    total = 0
    for i in range(200_000):
        text = f"item-{i}-{i + 1}"
        total += len(text)
    print(total)


if __name__ == "__main__":
    main()
