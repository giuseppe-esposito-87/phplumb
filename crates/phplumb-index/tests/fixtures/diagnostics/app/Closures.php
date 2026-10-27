<?php
namespace App;

class Hidden
{
    private function secret(): string
    {
        return 'x';
    }
}

class Outer
{
    public function run(): void
    {
        $fn = function () {
            return $this->secret();
        };
        \Closure::bind($fn, new Hidden(), Hidden::class)();
        $arrow = fn () => $this->secret();
        $scoped = function () {
            return self::hiddenStatic();
        };
        $this->/*@broken*/missingHere();
    }
}
