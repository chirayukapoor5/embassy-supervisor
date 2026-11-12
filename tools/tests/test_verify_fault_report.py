import json
import unittest
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from verify_fault_report import validate

class ReportValidatorTests(unittest.TestCase):
    def setUp(self):
        self.manifest={"requirements":[{"id":"SR-001","scenario":"FI-001"},{"id":"SR-002","scenario":"FI-002"}]}
        self.a={"type":"scenario","id":"FI-001","requirement":"SR-001","injection":"boot",
                "expected":"SAFE","observed":"SAFE","passed":True}
        self.b={"type":"scenario","id":"FI-002","requirement":"SR-002","injection":"overload",
                "expected":"BLOCK","observed":"BLOCK","passed":True}
        self.summary={"type":"summary","scenarios":2,"failed":0,
                      "method":"deterministic-host-model","hardware_executed":False}
    def lines(self,*obj): return [json.dumps(v) for v in obj]
    def test_valid(self):
        self.assertEqual(len(validate(self.manifest,self.lines(self.a,self.b,self.summary))),2)
    def test_missing(self):
        with self.assertRaises(ValueError): validate(self.manifest,self.lines(self.a,self.summary))
    def test_duplicate(self):
        with self.assertRaises(ValueError): validate(self.manifest,self.lines(self.a,self.a,self.b,self.summary))
    def test_mismatch(self):
        with self.assertRaises(ValueError):
            validate(self.manifest,self.lines(self.a,{**self.b,"observed":"ALLOW"},self.summary))
    def test_wrong_mapping(self):
        with self.assertRaises(ValueError):
            validate(self.manifest,self.lines({**self.a,"requirement":"SR-002"},self.b,self.summary))
    def test_false_hardware_claim(self):
        with self.assertRaises(ValueError):
            validate(self.manifest,self.lines(self.a,self.b,{**self.summary,"hardware_executed":True}))
