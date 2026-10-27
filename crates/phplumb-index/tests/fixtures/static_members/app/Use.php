<?php
namespace App;

use Lib\Thing;
use Lib\Suit;

Thing::/*@cursor:make*/make();
echo Thing::/*@cursor:const*/VERSION;
echo Thing::/*@cursor:sprop*/$count;
echo Suit::/*@cursor:case*/Hearts;
echo /*@cursor:nsconst*/LIMIT_NS;
echo /*@cursor:global*/GLOBAL_ONLY;
echo /*@cursor:defined*/DEFINED_ONE;
Unknown::/*@cursor:none*/foo();
Thing::/*@cursor:none*/missing();
