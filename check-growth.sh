#!/bin/sh

echo $'file\tbefore\tafter'
for file in out/*
do
  base=$(basename "$file")
  before=$(wc -c <"emots/$base")
  after=$(wc -c <"$file")
  printf '%s\t%s\t%s\n' "$base" "$before" "$after"
done
