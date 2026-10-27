<?php
namespace App;

abstract class Processor
{
    public function run(): void
    {
        if (is_callable([$this, 'preProcess'])) {
            $this->preProcess();
        }
        if (is_callable('opt_fn')) {
            opt_fn();
        }
        if (class_exists(class: \Lib\NamedOptional::class)) {
            new \Lib\NamedOptional();
        }
    }
}
