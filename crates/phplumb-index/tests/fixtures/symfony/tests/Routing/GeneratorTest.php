<?php

namespace App\Tests\Routing;

use Symfony\Component\Routing\Generator\UrlGenerator;

class GeneratorTest
{
    public function testGenerate(UrlGenerator $urls): void
    {
        $urls->generate('test_route');
    }
}
