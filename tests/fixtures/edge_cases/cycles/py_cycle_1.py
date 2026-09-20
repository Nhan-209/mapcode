# tests/fixtures/edge_cases/cycles/py_cycle_1.py
from .py_cycle_2 import helper_two

def helper_one():
    return helper_two() + " -> one"
