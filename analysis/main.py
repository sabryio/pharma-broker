"""
Master script to run all PharmaBroker data analysis phases.
Usage: python analysis/run_all.py
"""

import subprocess
import sys
from datetime import datetime
from pathlib import Path


def run_phase(script_path: Path, phase_num: int) -> bool:
    """Run a single analysis phase."""
    print(f"\n{'=' * 60}")
    print(f"🔍 Phase {phase_num}: {script_path.name}")
    print("=" * 60)

    result = subprocess.run(
        [sys.executable, str(script_path)], capture_output=False, cwd=script_path.parent
    )
    return result.returncode == 0


def main():
    script_dir = Path(__file__).parent
    scripts_dir = script_dir / "scripts"
    reports_dir = script_dir.parent / "reports"
    reports_dir.mkdir(exist_ok=True)

    # Find all numbered scripts (01-09) in scripts folder
    scripts = sorted(scripts_dir.glob("[0-9][0-9]_*.py"))

    print("=" * 60)
    print("📊 PharmaBroker Data Analysis")
    print(f"📅 {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 60)
    print(f"Found {len(scripts)} analysis phases to run")

    results = []
    for i, script in enumerate(scripts, 1):
        success = run_phase(script, i)
        results.append((i, script.name, "✅" if success else "❌"))

    # Summary
    print("\n" + "=" * 60)
    print("📋 ANALYSIS SUMMARY")
    print("=" * 60)

    passed = sum(1 for _, _, status in results if status == "✅")
    for num, name, status in results:
        print(f"  Phase {num}: {status} {name}")

    print(f"\n✅ Completed: {passed}/{len(results)} phases")
    print(f"📁 Reports saved to: {reports_dir}")

    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
