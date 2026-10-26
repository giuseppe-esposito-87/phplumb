<?php
namespace App;

use Lib\BasePolygon;
use Lib\Shape;
use Lib\Square;

function draw(/*@cursor:shape_impl*/Shape $shape, $unknown): void
{
    $shape->/*@cursor:run_impl*/run();
    $unknown->/*@cursor:none*/run();
}

function concrete(Square $square, BasePolygon $polygon): void
{
    $square->/*@cursor:sq_run*/run();
    $polygon->/*@cursor:bp_run*/run();
}
