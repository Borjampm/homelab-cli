#!/bin/bash
for i in $(seq 1 5); do
    echo "$(date +%H:%M:%S) - tick $i"
    sleep 0.5
done
echo "done"
