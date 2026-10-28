<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

class BlogController
{
    #[Route('/blog')]
    #[Route('/articles')]
    public function listAction(): void
    {
    }

    #[Route('/blog/{id}')]
    public function show(): void
    {
    }
}
