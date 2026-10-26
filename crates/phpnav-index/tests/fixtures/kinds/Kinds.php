<?php
namespace K;

interface /*@target:i*/Iface {}
trait /*@target:t*/Tr {}
enum /*@target:e*/En
{
    case A;
}
class /*@target:c*/Both {}
function /*@target:f*/Both() {}
class /*@target:self*/ /*@cursor:self*/User implements /*@cursor:i*/Iface
{
    use /*@cursor:t*/Tr;

    public function x(): /*@cursor:e*/En
    {
        return /*@cursor:e*/En::A;
    }
}
new /*@cursor:c*/Both();
/*@cursor:f*/Both();
