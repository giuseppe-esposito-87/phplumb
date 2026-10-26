<?php
namespace App;

use Lib\Shape;

function draw(/*@cursor:shape_impl*/Shape $shape, $unknown): void
{
    $shape->/*@cursor:run_impl*/run();
    $unknown->/*@cursor:none*/run();
}
