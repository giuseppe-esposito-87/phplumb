<?php

namespace App\Tests\Twig;

use Twig\Environment;
use Twig\Loader\ArrayLoader;

class ExtensionTest
{
    public function testRender(): void
    {
        (new Environment(new ArrayLoader(['index' => 'x'])))->render('index');
    }
}
