#!/opt/local/bin/bash
# one engine at a time: engine lock, owner unique, released always
source $QUANTBENCH/scripts/lib/engine_lock.sh
cd $LIVE
engine_lock_acquire ArleFork-boolean-tool-args 1800 || { echo "lock not acquired"; exit 3; }
trap 'engine_lock_release' EXIT
MODEL=$MODEL_CACHE/models--mlx-community--Qwen3.5-4B-4bit/snapshots/0e7ffd5c629ef7719d4cbc04069232580bfa9d9c
run() { # label binary port
  local label=$1 bin=$2 port=$3
  sudo -n -u $ENGINE_USER "$bin" serve --backend metal --model-path $MODEL --bind 127.0.0.1 --port $port --metal-pipeline false --metal-paged-kv-read false > serve-$label.log 2>&1 &
  local pid=$!
  for i in $(seq 1 120); do curl -sf http://127.0.0.1:$port/v1/models >/dev/null 2>&1 && break; sleep 2; done
  curl -sf http://127.0.0.1:$port/v1/models | head -c 300; echo
  (cd $QUANTBENCH && env PYTHONPATH=src /opt/local/bin/python3.14 $LIVE/probe.py http://127.0.0.1:$port $label) 
  sudo -n -u $ENGINE_USER pkill -f "serve --backend metal --model-path $MODEL --bind 127.0.0.1 --port $port"
  wait $pid 2>/dev/null
  echo "$label: engine exited"
}
run patched $SCRATCH/target-arle/release/arle 14861
run base /opt/local/libexec/arle/arle 14862
