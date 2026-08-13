.PHONY: dataset train test serve package web desktop all

dataset:
	python3 train/build_dataset.py

train: dataset
	python3 train/train_lm.py --steps 800

test:
	python3 -m unittest discover -s tests -v

serve:
	python3 -m uvicorn api.server:app --host 0.0.0.0 --port 8080

web:
	python3 scripts/build_webapp.py

desktop:
	cd desktop && cargo build --release

package: web desktop
	./scripts/package.sh

all: train test serve
