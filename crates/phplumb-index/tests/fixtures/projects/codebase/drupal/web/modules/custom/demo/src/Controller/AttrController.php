<?php

namespace Drupal\demo\Controller;

use Symfony\Component\Routing\Attribute\Route;

class AttrController
{
    public function page(): array
    {
        return [];
    }

    #[Route('/demo/attr', name: 'demo.attr')]
    public function attr(): array
    {
        return [];
    }
}
