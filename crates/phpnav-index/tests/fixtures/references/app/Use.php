<?php
namespace App;

use /*@target:entity*/Lib\Entity;
use Lib\Saveable;
use Lib\Unrelated;
use function /*@target:helper*/Lib\helper;

function run(/*@target:entity*/Entity $e, Saveable $s, Unrelated $u, $unknown): void
{
    $e->/*@target:save*/save();
    $s->/*@target:save*/save();
    $u->save();
    $unknown->save();
    new /*@target:entity*/Entity();
    if ($s instanceof /*@target:entity*/Entity) {
    }
    /*@target:entity*/Entity::/*@target:make*/make();
    echo /*@target:limit*/\Lib\LIMIT;
    /*@target:helper*/helper();
    /*@target:helper*/\Lib\helper();
    /*@target:ghelper*/ghelper();
}
