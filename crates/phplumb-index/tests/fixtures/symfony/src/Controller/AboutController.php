<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

#[Route('/about', name: 'about')]
class AboutController
{
    public function __invoke(): void
    {
    }
}
