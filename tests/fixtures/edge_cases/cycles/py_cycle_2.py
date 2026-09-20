# tests/fixtures/edge_cases/cycles/py_cycle_2.py
from .py_cycle_1 import helper_one

def helper_two():
    return "two"
