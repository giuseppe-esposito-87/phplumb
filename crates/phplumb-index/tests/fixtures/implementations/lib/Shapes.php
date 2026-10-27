<?php
namespace Lib;

interface /*@cursor:shape_impl*/Shape
{
    public function /*@cursor:run_impl*/run(): void;
}

interface Polygon extends Shape
{
}

trait Runs
{
    public function /*@target:run_impl*/run(): void
    {
    }
}

abstract class BasePolygon implements Polygon
{
    public function /*@target:run_impl*/ /*@target:sq_run*/ /*@target:bp_run*/run(): void
    {
    }
}

class /*@target:shape_impl*/Square extends BasePolygon
{
}

class /*@target:shape_impl*/Cube extends Square
{
    public function /*@target:run_impl*/ /*@target:sq_run*/ /*@target:bp_run*/run(): void
    {
    }
}

class /*@target:shape_impl*/Circle implements Shape
{
    use Runs;
}

class /*@target:shape_impl*/Oval implements Shape
{
    use Runs;
}

class /*@target:shape_impl*/Hexagon extends BasePolygon
{
    public function /*@target:run_impl*/ /*@target:bp_run*/run(): void
    {
    }
}

class /*@cursor:none*/Lonely
{
}
