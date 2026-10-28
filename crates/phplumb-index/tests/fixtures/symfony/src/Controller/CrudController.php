<?php

namespace App\Controller;

use Symfony\Component\Routing\Attribute\Route;

abstract class CrudController
{
    #[Route('/', name: 'index')]
    public function index(): void
    {
    }

    #[Route('/list')]
    public function list(): void
    {
    }
}
