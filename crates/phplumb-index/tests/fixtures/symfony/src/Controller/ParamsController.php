<?php

namespace App\Controller;

use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;

class ParamsController extends AbstractController
{
    public function params(): void
    {
        $this->getParameter('app.limit');
        $this->getParameter('kernel.project_dir');
    }
}
