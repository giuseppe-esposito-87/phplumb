<?php

namespace App;

class Uses
{
    public function run(): string
    {
        try {
            $length = count([]);
        } catch (\RuntimeException $e) {
            return $e->getMessage();
        }
        $year = (new \DateTimeImmutable())->modify('+1 day')->format('Y');

        return $year . \PHP_EOL . $length;
    }

    public function text(\Stringable $value): string
    {
        return (string) $value;
    }
}

class AppException extends \RuntimeException
{
    public function __construct()
    {
        parent::__construct('failed');
    }

    public function describe(): string
    {
        return $this->getMessage();
    }
}
