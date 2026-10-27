<?php
declare(strict_types=1);

namespace App;

use Lib\Content;
use Lib\Missing as Gone;
use function Lib\nothing_imported;
use const Lib\NOTHING_IMPORTED;

#[\Lib\NoSuchAttribute(flag: true)]
class Uses extends /*@broken*/Absent implements /*@broken*/AbsentInterface, \Countable
{
    use /*@broken*/AbsentTrait;

    public /*@broken*/Shadow $typed;

    public function count(): int
    {
        return 0;
    }

    public function types(/*@broken*/Undefined $a, ?Content $b, int|/*@broken*/Other $c): /*@broken*/Nowhere
    {
        /** @var \Lib\NoSuchDoc $doc */
        $doc = null;
        new /*@broken*/Ghost();
        new /*@broken*/Gone();
        new \DateTime();
        new /*@broken*/DateTime();
        new \Lib\Content();
        $class = Ghost::class;
        if ($a instanceof /*@broken*/Phantom) {
        }
        try {
        } catch (/*@broken*/Lost | \RuntimeException $e) {
        }
        /*@broken*/Spectre::run();
        echo /*@broken*/Wraith::VALUE;
        str_pad(string: 'x', length: 3);
        return new /*@broken*/Nowhere();
    }

    public function calls(): void
    {
        \Lib\helper();
        /*@broken*/\Lib\nothing();
        global_helper();
        strlen('x');
        \strlen('x');
        /*@broken*/invented();
        /*@broken*/nothing_imported();
        echo \Lib\LIMIT, GLOBAL_LIMIT, DEFINED_LIMIT, PHP_EOL, \E_ALL, true, null;
        echo /*@broken*/UNDEFINED_LIMIT, /*@broken*/\Lib\NOPE, /*@broken*/NOTHING_IMPORTED;
    }
}
