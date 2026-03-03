#!/bin/bash
for i in $(seq 1 5000); do
    echo "line $i: $(head -c 80 /dev/urandom | base64 | head -c 80)"
done
echo "DONE: printed 5000 lines"
