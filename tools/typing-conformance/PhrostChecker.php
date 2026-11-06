<?php

declare(strict_types=1);

namespace Conformance\Checker;

use Conformance\Discovery\TestCase;
use RuntimeException;

/**
 * phrost's `check` CLI, the same engine its language server runs.
 *
 * The test directory is checked once, as a project, and each test case reads
 * its own file's findings out of that run: every test declares its own
 * namespace, so the files do not collide, and one index over 250 files is
 * what an editor would hold anyway. The type checks phrost leaves off by
 * default are switched on in the copy's `phrost.toml`.
 */
final class PhrostChecker implements Checker
{
    /** @var array<string, array<int, list<string>>>|null */
    private ?array $byFile = null;

    public function __construct(
        private readonly string $binaryPath,
        private readonly string $testsDir,
    ) {
    }

    public function name(): string
    {
        return 'phrost';
    }

    public function version(): string
    {
        exec(escapeshellarg($this->binaryPath) . ' --version 2>&1', $output, $exitCode);
        if ($exitCode !== 0) {
            throw new RuntimeException('Failed to determine phrost version.');
        }

        return trim(implode("\n", $output));
    }

    /**
     * @return array<int, list<string>>
     */
    public function analyse(TestCase $testCase): array
    {
        $this->byFile ??= $this->checkAll();
        $diagnostics = $this->byFile[$testCase->fileName] ?? [];
        ksort($diagnostics);

        return $diagnostics;
    }

    /**
     * @return array<string, array<int, list<string>>>
     */
    private function checkAll(): array
    {
        $workspace = sys_get_temp_dir() . '/phrost-conformance-' . getmypid();
        if (!is_dir($workspace) && !mkdir($workspace, 0777, true)) {
            throw new RuntimeException('Cannot create ' . $workspace);
        }
        foreach (glob($this->testsDir . '/*.php') ?: [] as $file) {
            copy($file, $workspace . '/' . basename($file));
        }
        file_put_contents(
            $workspace . '/phrost.toml',
            "[lint]\non = [\"possibly-absent-key\", \"extra-argument\"]\n",
        );

        $command = sprintf('cd %s && %s check . 2>/dev/null', escapeshellarg($workspace), escapeshellarg($this->binaryPath));
        exec($command, $output, $exitCode);

        $byFile = [];
        foreach ($output as $line) {
            if (preg_match('/^(.+?\.php):(\d+):\d+: \w+ ([\w-]+): (.*)$/', $line, $match) !== 1) {
                continue;
            }
            $byFile[basename($match[1])][(int) $match[2]][] = sprintf('%s [%s]', trim($match[4]), $match[3]);
        }

        return $byFile;
    }
}
