"""Run one real unittest module with structured discovery/result/skip evidence."""
from pathlib import Path
import argparse
import importlib.util
import json
import os
import sys
import unittest
import worldwright_local as local
from r6_support import encode, new_file


def names(suite):
    result = []
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            result.extend(names(item))
        else:
            result.append(item.id())
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--file', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    local.configure_tools_environment()
    path = local.reject_links(args.file).resolve(strict=True)
    if not path.is_relative_to(local.ROOT.resolve()) or path.suffix != '.py':
        parser.error('Python source inside repository required')
    output = local.local_path(args.report)
    if output.exists() or not output.parent.is_dir():
        parser.error('new report with existing local parent required')
    value = {'schema_version': 1, 'kind': 'python-unittest-execution', 'file': str(path),
             'status': 'failed', 'tests': [], 'tests_run': 0, 'full_suite_passed': False}
    try:
        sys.path.insert(0, str(path.parent))
        sys.argv = [str(path)]
        spec = importlib.util.spec_from_file_location('audited_unit', path)
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        loader = unittest.TestLoader()
        suite = loader.loadTestsFromModule(module)
        value['tests'] = names(suite)
        if loader.errors or not value['tests'] or len(set(value['tests'])) != len(value['tests']):
            raise ValueError('empty, duplicate or failed unittest discovery')
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        value.update(tests_run=result.testsRun,
                     failures=[str(t) for t, _ in result.failures],
                     errors=[str(t) for t, _ in result.errors],
                     skips=[{'test': str(t), 'reason': reason} for t, reason in result.skipped],
                     expected_failures=[str(t) for t, _ in result.expectedFailures],
                     unexpected_successes=[str(t) for t in result.unexpectedSuccesses])
        if result.wasSuccessful() and result.testsRun == len(value['tests']):
            value['status'] = 'passed' if not result.skipped and not result.expectedFailures else 'executed-with-unverified-cases'
    except Exception as error:
        value['failure_type'] = type(error).__name__
        print('Unittest setup/execution failed:', type(error).__name__, file=sys.stderr)
    new_file(output, encode(value))
    return 0 if value['status'] == 'passed' else 2 if value['status'] == 'executed-with-unverified-cases' else 1


if __name__ == '__main__':
    raise SystemExit(main())
