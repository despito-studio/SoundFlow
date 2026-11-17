'use strict';
(()=>{
 const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;
 if(!reduced&&'IntersectionObserver'in window){const observer=new IntersectionObserver(entries=>entries.forEach(entry=>{if(entry.isIntersecting){entry.target.classList.add('revealed');observer.unobserve(entry.target)}}),{threshold:.08});document.querySelectorAll('.feature,.island-section,.island-band,.real-app,.direct-download,.download-section,.cards article').forEach(el=>{observer.observe(el)});}
 // Download build IDs are embedded in the filename, so browser caches cannot
 // silently serve a different preview under the same generic URL.
 document.querySelectorAll('a[download]').forEach(a=>a.addEventListener('click',()=>{let note=document.getElementById('download-status');if(!note){note=document.createElement('p');note.id='download-status';note.className='new-download-message';note.setAttribute('role','status');a.parentElement.append(note)}note.textContent=document.documentElement.lang==='uk'?'Завантажується актуальна локальна збірка. Після завантаження закрий SoundFlow перед встановленням.':'Downloading the current local build. Exit SoundFlow before running the installer.';}));
})();
