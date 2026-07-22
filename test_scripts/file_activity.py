from pathlib import Path

Path("trace_test_output.txt").write_text("hello")
print("created trace_test_output.txt")