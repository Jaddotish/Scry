with open("huge.bin", "wb") as f:
    while True:
        f.write(b"x" * 1_000_000)