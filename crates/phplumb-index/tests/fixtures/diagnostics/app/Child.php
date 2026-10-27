<?php
namespace App;

class Child extends \Lib\Shared
{
    public function run(): void
    {
        $this->real();
        $this->/*@broken*/unreal();
    }
}
