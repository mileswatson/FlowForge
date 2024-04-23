import os

for i in reversed(["0.1", "1", "10", "100"]):
    os.system(f"cargo run --features cuda --release train -c configs\\trainer\\remyr\\default.json --net configs\\network\\remy\\default.json --util configs\\utility\\delta{i}.json --dna trained/remyr/new2/delta{i}/delta{i}.remyr.dna --eval configs\\eval\\very_short.json --eval-times 100 --progress trained/remyr/new2/delta{i}/trainout.json")
