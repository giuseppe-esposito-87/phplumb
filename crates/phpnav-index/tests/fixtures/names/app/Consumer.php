<?php
namespace App;

use /*@cursor:thing*/Lib\Thing;
use Lib\Thing as Alias;
use Lib\ThingInterface;
use Lib\BaseThing;
use Lib\ThingException;
use Lib\Marker;

#[/*@cursor:attr*/Marker]
class Consumer extends /*@cursor:base*/BaseThing implements /*@cursor:iface*/ThingInterface
{
    private /*@cursor:thing*/Thing $prop;

    public function run(/*@cursor:thing*/Thing $param): /*@cursor:thing*/Thing
    {
        $a = new /*@cursor:thing*/Alias();
        $b = new /*@cursor:same*/Same();
        $c = new /*@cursor:thing*/\Lib\Thing();
        $d = /*@cursor:thing*/Thing::make();
        if ($a instanceof /*@cursor:thing*/Thing) {
        }
        try {
        } catch (/*@cursor:exc*/ThingException $e) {
        }
        $f = new /*@cursor:thing*/thing();
        $g = new /*@cursor:thing*/\lib\THING();
        return $d;
    }
}
