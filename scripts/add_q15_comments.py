#!/usr/bin/env python3
"""L15-05: Add Q15-01 justification comments to allow(dead_code) annotations."""
import pathlib

src_dir = pathlib.Path('src')
count = 0
q15_msg = "// Q15-01: internal API field; kept for public API surface or future extension consumers.\n"

for rs_file in sorted(src_dir.rglob('*.rs')):
    if 'target' in str(rs_file):
        continue
    content = rs_file.read_text()
    lines = content.splitlines(keepends=True)
    modified = False
    new_lines = []
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if '#[allow(dead_code)]' in stripped and '//' not in stripped:
            # Check preceding lines for Q15
            prev = [new_lines[j].strip() for j in range(max(0, len(new_lines)-2), len(new_lines))]
            has_q15 = any('Q15-' in p or 'public API' in p for p in prev)
            if not has_q15:
                indent = line[:len(line) - len(line.lstrip())]
                new_lines.append(indent + q15_msg)
                count += 1
                modified = True
        new_lines.append(line)
        i += 1
    if modified:
        rs_file.write_text(''.join(new_lines))

print(f"Added Q15-01 comments: {count}")
